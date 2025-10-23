using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text;
using System.Xml.Linq;

namespace OpenDraft.XSVG
{
    internal struct ODSvgStyle
    {
        public ODColour StrokeColor;
        public ODColour FillColor;
        public double StrokeWidth;
        public string LineType;
        public string LayerName;
        public bool IsVisible;
        public bool IsLocked;
    }

    public class CADLayer
    {
        public string Id { get; set; }
        public string Name { get; set; }
        public ODColour Color { get; set; }
        public bool Visible { get; set; }
        public bool Locked { get; set; }
        public double LineWeight { get; set; }
        public string LineType { get; set; }
    }

    public class ODSvgReader
    {
        private string? _filePath;
        private XDocument? _xDocument;
        private XNamespace _ns = "http://www.w3.org/2000/svg";
        private XNamespace _cadNs = "http://your-cad-app.org/cad/1.0";
        private XNamespace _dcNs = "http://purl.org/dc/elements/1.1/";

        private Dictionary<string, CADLayer> _layers = new Dictionary<string, CADLayer>();
        private Dictionary<string, XElement> _blockDefinitions = new Dictionary<string, XElement>();
        private CADMetadata _metadata = new CADMetadata();

        public void LoadSvgFile(string filePath)
        {
            _filePath = filePath;

            if (!System.IO.File.Exists(filePath))
                throw new System.IO.FileNotFoundException("SVG file not found", filePath);

            string fileContentsRaw = System.IO.File.ReadAllText(filePath);
            _xDocument = XDocument.Parse(fileContentsRaw);

            // Parse CAD-specific metadata and definitions
            ParseCADMetadata();
            ParseLayerDefinitions();
            ParseBlockDefinitions();
        }

        #region CAD Metadata Parsing

        private void ParseCADMetadata()
        {
            if (_xDocument?.Root == null) return;

            // Parse root CAD attributes
            _metadata.AppVersion = _xDocument.Root.Attribute(_cadNs + "app-version")?.Value;
            _metadata.FileVersion = _xDocument.Root.Attribute(_cadNs + "file-version")?.Value;
            _metadata.Units = _xDocument.Root.Attribute(_cadNs + "units")?.Value;
            _metadata.Precision = ParseDouble(_xDocument.Root.Attribute(_cadNs + "precision")?.Value) ?? 0.001;

            // Parse metadata section
            var metadataElement = _xDocument.Root.Element("metadata");
            if (metadataElement != null)
            {
                _metadata.Application = metadataElement.Element(_cadNs + "application")?.Value;
                _metadata.Version = metadataElement.Element(_cadNs + "version")?.Value;
                _metadata.Created = ParseDateTime(metadataElement.Element(_cadNs + "created")?.Value);
                _metadata.Modified = ParseDateTime(metadataElement.Element(_cadNs + "modified")?.Value);

                // Parse Dublin Core metadata
                _metadata.Title = metadataElement.Element(_dcNs + "title")?.Value;
                _metadata.Creator = metadataElement.Element(_dcNs + "creator")?.Value;
                _metadata.Description = metadataElement.Element(_dcNs + "description")?.Value;

                // Parse custom properties
                var propertiesElement = metadataElement.Element(_cadNs + "properties");
                if (propertiesElement != null)
                {
                    _metadata.Properties = new Dictionary<string, string>();
                    foreach (var propElement in propertiesElement.Elements(_cadNs + "property"))
                    {
                        var name = propElement.Attribute("name")?.Value;
                        var value = propElement.Attribute("value")?.Value;
                        if (name != null)
                        {
                            _metadata.Properties[name] = value ?? string.Empty;
                        }
                    }
                }
            }
        }

        private void ParseLayerDefinitions()
        {
            var layersElement = _xDocument?.Root?
                .Element("defs")?
                .Element(_cadNs + "layers");

            if (layersElement == null) return;

            foreach (var layerElement in layersElement.Elements(_cadNs + "layer"))
            {
                var layer = new CADLayer
                {
                    Id = layerElement.Attribute("id")?.Value,
                    Name = layerElement.Attribute("name")?.Value,
                    Color = ParseColor(layerElement.Attribute("color")?.Value),
                    Visible = layerElement.Attribute("visible")?.Value == "true",
                    Locked = layerElement.Attribute("locked")?.Value == "true",
                    LineWeight = ParseDouble(layerElement.Attribute("lineweight")?.Value) ?? 0.25,
                    LineType = layerElement.Attribute("linetype")?.Value ?? "continuous"
                };

                if (layer.Id != null)
                {
                    _layers[layer.Id] = layer;
                }
            }
        }

        private void ParseBlockDefinitions()
        {
            var defsElement = _xDocument?.Root?.Element("defs");
            if (defsElement == null) return;

            foreach (var symbolElement in defsElement.Elements(_ns + "symbol"))
            {
                var id = symbolElement.Attribute("id")?.Value;
                if (id != null)
                {
                    _blockDefinitions[id] = symbolElement;
                }
            }
        }

        #endregion

        #region Geometry Import

        public List<ODElement> ImportGeometry()
        {
            if (_xDocument == null)
                throw new InvalidOperationException("No SVG file loaded. Call LoadSvgFile first.");

            var geometries = new List<ODElement>();
            var svgElement = _xDocument.Root;

            // Process groups (layers) first
            geometries.AddRange(ProcessGroups(svgElement));

            // Import individual elements not in groups
            geometries.AddRange(ImportPaths(svgElement));
            geometries.AddRange(ImportRectangles(svgElement));
            geometries.AddRange(ImportLines(svgElement));
            geometries.AddRange(ImportCircles(svgElement));
            geometries.AddRange(ImportBlockReferences(svgElement));

            Debug.WriteLine($"Imported {geometries.Count} geometry objects from {_layers.Count} layers");
            return geometries;
        }

        private List<ODElement> ProcessGroups(XElement parentElement)
        {
            var elements = new List<ODElement>();

            foreach (var groupElement in parentElement.Elements(_ns + "g"))
            {
                var layerId = groupElement.Attribute(_cadNs + "layer-id")?.Value;
                var style = ParseStyle(groupElement, layerId);

                // Process elements within the group
                elements.AddRange(ImportPaths(groupElement, style));
                elements.AddRange(ImportRectangles(groupElement, style));
                elements.AddRange(ImportLines(groupElement, style));
                elements.AddRange(ImportCircles(groupElement, style));
                elements.AddRange(ImportBlockReferences(groupElement, style));
                elements.AddRange(ProcessGroups(groupElement)); // Recursive for nested groups
            }

            return elements;
        }

        private List<ODElement> ImportBlockReferences(XElement parentElement, ODSvgStyle? parentStyle = null)
        {
            var blocks = new List<ODElement>();

            foreach (var useElement in parentElement.Descendants(_ns + "use"))
            {
                var href = useElement.Attribute(XNamespace.Get("http://www.w3.org/1999/xlink") + "href")?.Value;
                if (!string.IsNullOrEmpty(href) && href.StartsWith("#"))
                {
                    var blockId = href.Substring(1);
                    if (_blockDefinitions.TryGetValue(blockId, out var blockDefinition))
                    {
                        var style = ParseStyle(useElement, null, parentStyle);
                        var x = ParseDouble(useElement.Attribute("x")?.Value) ?? 0;
                        var y = ParseDouble(useElement.Attribute("y")?.Value) ?? 0;

                        // Parse transformation
                        var transform = useElement.Attribute("transform")?.Value;
                        var rotation = ParseDouble(useElement.Attribute(_cadNs + "rotation")?.Value) ?? 0;

                        var blockElements = ParseBlockDefinition(blockDefinition, x, y, rotation, style);
                        foreach (var element in blockElements)
                        {
                            ApplyCADMetadata(element, useElement);
                            blocks.Add(element);
                        }
                    }
                }
            }

            return blocks;
        }

        private List<ODElement> ParseBlockDefinition(XElement blockDefinition, double x, double y, double rotation, ODSvgStyle style)
        {
            var elements = new List<ODElement>();

            // Process paths in block definition
            foreach (var pathElement in blockDefinition.Descendants(_ns + "path"))
            {
                var pathData = pathElement.Attribute("d")?.Value;
                if (!string.IsNullOrEmpty(pathData))
                {
                    var path = ParsePathData(pathData, style);
                    if (path != null)
                    {
                        // Apply block transformation
                        ApplyTransformation(path, x, y, rotation);
                        elements.Add(path);
                    }
                }
            }

            // Process rectangles in block definition
            foreach (var rectElement in blockDefinition.Descendants(_ns + "rect"))
            {
                var rectX = ParseDouble(rectElement.Attribute("x")?.Value) ?? 0;
                var rectY = ParseDouble(rectElement.Attribute("y")?.Value) ?? 0;
                var width = ParseDouble(rectElement.Attribute("width")?.Value) ?? 0;
                var height = ParseDouble(rectElement.Attribute("height")?.Value) ?? 0;

                var rectangle = new ODRectangle(
                    new ODVec2(rectX + x, rectY + y),
                    new ODVec2(rectX + x + width, rectY + y + height)
                );
                ApplyTransformation(rectangle, x, y, rotation);
                elements.Add(rectangle);
            }

            return elements;
        }

        private void ApplyTransformation(ODElement element, double x, double y, double rotation)
        {
            // This is a simplified transformation - you'll need to implement
            // proper transformation logic based on your ODElement hierarchy
            if (element is ODPolyline polyline)
            {
                for (int i = 0; i < polyline.Points.Count; i++)
                {
                    var point = polyline.Points[i];
                    // Apply translation and rotation here
                    polyline.Points[i] = new ODVec2(point.X + x, point.Y + y);
                }
            }
            // Add similar logic for other element types
        }

        #endregion

        #region Original Geometry Import Methods (Preserved)

        private List<ODElement> ImportPaths(XElement parentElement, ODSvgStyle? parentStyle = null)
        {
            var paths = new List<ODElement>();

            foreach (var pathElement in parentElement.Descendants(_ns + "path"))
            {
                var pathData = pathElement.Attribute("d")?.Value;
                if (!string.IsNullOrEmpty(pathData))
                {
                    var style = ParseStyle(pathElement, null, parentStyle);
                    var path = ParsePathData(pathData, style);
                    if (path != null)
                    {
                        ApplyCADMetadata(path, pathElement);
                        paths.Add(path);
                    }
                }
            }

            return paths;
        }

        private List<ODElement> ImportRectangles(XElement parentElement, ODSvgStyle? parentStyle = null)
        {
            var rectangles = new List<ODElement>();

            foreach (var rectElement in parentElement.Descendants(_ns + "rect"))
            {
                var x = ParseDouble(rectElement.Attribute("x")?.Value) ?? 0;
                var y = ParseDouble(rectElement.Attribute("y")?.Value) ?? 0;
                var width = ParseDouble(rectElement.Attribute("width")?.Value) ?? 0;
                var height = ParseDouble(rectElement.Attribute("height")?.Value) ?? 0;

                if (width > 0 && height > 0)
                {
                    var style = ParseStyle(rectElement, null, parentStyle);
                    var rectangle = new ODRectangle(
                        new ODVec2(x, y),
                        new ODVec2(x + width, y + height)
                    );
                    ApplyCADMetadata(rectangle, rectElement);
                    rectangles.Add(rectangle);
                }
            }

            return rectangles;
        }

        private List<ODElement> ImportLines(XElement parentElement, ODSvgStyle? parentStyle = null)
        {
            var lines = new List<ODElement>();

            foreach (var lineElement in parentElement.Descendants(_ns + "line"))
            {
                var x1 = ParseDouble(lineElement.Attribute("x1")?.Value) ?? 0;
                var y1 = ParseDouble(lineElement.Attribute("y1")?.Value) ?? 0;
                var x2 = ParseDouble(lineElement.Attribute("x2")?.Value) ?? 0;
                var y2 = ParseDouble(lineElement.Attribute("y2")?.Value) ?? 0;

                var style = ParseStyle(lineElement, null, parentStyle);
                var line = new ODLine(
                    new ODVec2(x1, y1),
                    new ODVec2(x2, y2)
                );
                ApplyCADMetadata(line, lineElement);
                lines.Add(line);
            }

            return lines;
        }

        private List<ODElement> ImportCircles(XElement parentElement, ODSvgStyle? parentStyle = null)
        {
            var circles = new List<ODElement>();

            foreach (var circleElement in parentElement.Descendants(_ns + "circle"))
            {
                var cx = ParseDouble(circleElement.Attribute("cx")?.Value) ?? 0;
                var cy = ParseDouble(circleElement.Attribute("cy")?.Value) ?? 0;
                var r = ParseDouble(circleElement.Attribute("r")?.Value) ?? 0;

                if (r > 0)
                {
                    var style = ParseStyle(circleElement, null, parentStyle);
                    var circle = new ODCircle(
                        new ODVec2(cx, cy),
                        r
                    );
                    ApplyCADMetadata(circle, circleElement);
                    circles.Add(circle);
                }
            }

            return circles;
        }

        #endregion

        #region Path Data Parsing (Original Methods)

        private ODElement ParsePathData(string pathData, ODSvgStyle style)
        {
            try
            {
                var points = new List<ODVec2>();
                var tokens = TokenizePathData(pathData);

                double currentX = 0, currentY = 0;
                double startX = 0, startY = 0;
                bool hasValidData = false;

                for (int i = 0; i < tokens.Count; i++)
                {
                    var token = tokens[i];
                    var upperToken = token.ToUpper();

                    switch (upperToken)
                    {
                        case "M": // Move To
                            if (i + 2 < tokens.Count &&
                                TryParseDouble(tokens[i + 1], out double mx) &&
                                TryParseDouble(tokens[i + 2], out double my))
                            {
                                if (token == "m") // relative
                                {
                                    currentX += mx;
                                    currentY += my;
                                }
                                else // absolute
                                {
                                    currentX = mx;
                                    currentY = my;
                                }
                                startX = currentX;
                                startY = currentY;
                                points.Add(new ODVec2(currentX, currentY));
                                i += 2;
                                hasValidData = true;
                            }
                            break;

                        case "L": // Line To
                            if (i + 2 < tokens.Count &&
                                TryParseDouble(tokens[i + 1], out double lx) &&
                                TryParseDouble(tokens[i + 2], out double ly))
                            {
                                if (token == "l") // relative
                                {
                                    currentX += lx;
                                    currentY += ly;
                                }
                                else // absolute
                                {
                                    currentX = lx;
                                    currentY = ly;
                                }
                                points.Add(new ODVec2(currentX, currentY));
                                i += 2;
                                hasValidData = true;
                            }
                            break;

                        case "H": // Horizontal Line To
                            if (i + 1 < tokens.Count &&
                                TryParseDouble(tokens[i + 1], out double hx))
                            {
                                if (token == "h") // relative
                                {
                                    currentX += hx;
                                }
                                else // absolute
                                {
                                    currentX = hx;
                                }
                                points.Add(new ODVec2(currentX, currentY));
                                i += 1;
                                hasValidData = true;
                            }
                            break;

                        case "V": // Vertical Line To
                            if (i + 1 < tokens.Count &&
                                TryParseDouble(tokens[i + 1], out double vy))
                            {
                                if (token == "v") // relative
                                {
                                    currentY += vy;
                                }
                                else // absolute
                                {
                                    currentY = vy;
                                }
                                points.Add(new ODVec2(currentX, currentY));
                                i += 1;
                                hasValidData = true;
                            }
                            break;

                        case "Z": // Close Path
                            if (points.Count > 0)
                            {
                                // Close the path by returning to start point
                                points.Add(new ODVec2(startX, startY));
                                hasValidData = true;
                            }
                            break;

                        default:
                            // Skip unknown commands and their parameters
                            if (IsCommandToken(token))
                            {
                                // Skip the command and its parameters
                                i += GetParameterCountForCommand(token);
                            }
                            break;
                    }
                }

                if (hasValidData && points.Count > 1)
                {
                    return new ODPolyline(points);
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error parsing path data: {ex.Message}");
                Debug.WriteLine($"Path data: {pathData}");
            }

            return null;
        }

        private List<string> TokenizePathData(string pathData)
        {
            var tokens = new List<string>();
            if (string.IsNullOrEmpty(pathData)) return tokens;

            var currentToken = new StringBuilder();
            bool inNumber = false;

            foreach (char c in pathData)
            {
                if (char.IsWhiteSpace(c) || c == ',')
                {
                    if (currentToken.Length > 0)
                    {
                        tokens.Add(currentToken.ToString());
                        currentToken.Clear();
                    }
                    inNumber = false;
                }
                else if (char.IsLetter(c) && c != '.' && c != '-')
                {
                    if (currentToken.Length > 0)
                    {
                        tokens.Add(currentToken.ToString());
                        currentToken.Clear();
                    }
                    currentToken.Append(c);
                    tokens.Add(currentToken.ToString());
                    currentToken.Clear();
                    inNumber = false;
                }
                else if (c == '-' && inNumber)
                {
                    // Negative sign in the middle of tokens indicates a new number
                    if (currentToken.Length > 0)
                    {
                        tokens.Add(currentToken.ToString());
                        currentToken.Clear();
                    }
                    currentToken.Append(c);
                    inNumber = true;
                }
                else
                {
                    // Number or decimal point or negative sign at start
                    currentToken.Append(c);
                    inNumber = true;
                }
            }

            // Add the last token
            if (currentToken.Length > 0)
            {
                tokens.Add(currentToken.ToString());
            }

            return tokens;
        }

        private bool TryParseDouble(string value, out double result)
        {
            // Handle culture-specific formatting
            return double.TryParse(value,
                System.Globalization.NumberStyles.Float,
                System.Globalization.CultureInfo.InvariantCulture,
                out result);
        }

        private bool IsCommandToken(string token)
        {
            if (string.IsNullOrEmpty(token)) return false;

            char firstChar = token[0];
            return "MLHVCSQTAZ".Contains(char.ToUpper(firstChar));
        }

        private int GetParameterCountForCommand(string command)
        {
            if (string.IsNullOrEmpty(command)) return 0;

            return char.ToUpper(command[0]) switch
            {
                'M' or 'L' or 'T' => 2,  // Move, Line, Smooth Quadratic
                'H' or 'V' => 1,         // Horizontal, Vertical
                'S' or 'Q' => 4,         // Smooth Curve, Quadratic
                'C' => 6,                // Curve
                'A' => 7,                // Arc
                'Z' => 0,                // Close path
                _ => 0
            };
        }

        #endregion

        #region Enhanced Style Parsing

        private ODSvgStyle ParseStyle(XElement element, string? layerId = null, ODSvgStyle? parentStyle = null)
        {
            var style = parentStyle ?? new ODSvgStyle();

            // Apply layer style if layer ID is provided
            if (layerId != null && _layers.TryGetValue(layerId, out var layer))
            {
                style.StrokeColor = layer.Color;
                style.StrokeWidth = layer.LineWeight;
                style.LineType = layer.LineType;
                style.LayerName = layer.Name;
                style.IsVisible = layer.Visible;
                style.IsLocked = layer.Locked;
            }

            // Parse element-specific style attributes
            var styleAttr = element.Attribute("style")?.Value;
            if (!string.IsNullOrEmpty(styleAttr))
            {
                var styleParts = styleAttr.Split(';');
                foreach (var part in styleParts)
                {
                    var keyValue = part.Split(':');
                    if (keyValue.Length == 2)
                    {
                        var key = keyValue[0].Trim();
                        var value = keyValue[1].Trim();

                        switch (key)
                        {
                            case "stroke":
                                style.StrokeColor = ParseColor(value);
                                break;
                            case "stroke-width":
                                if (ParseDouble(value) is double strokeWidth)
                                    style.StrokeWidth = strokeWidth;
                                break;
                            case "stroke-dasharray":
                                style.LineType = ParseLineType(value);
                                break;
                            case "fill":
                                style.FillColor = ParseColor(value);
                                break;
                        }
                    }
                }
            }

            // Parse individual attributes (override style attribute)
            var strokeAttr = element.Attribute("stroke")?.Value;
            if (!string.IsNullOrEmpty(strokeAttr))
                style.StrokeColor = ParseColor(strokeAttr);

            var strokeWidthAttr = element.Attribute("stroke-width")?.Value;
            if (!string.IsNullOrEmpty(strokeWidthAttr) && ParseDouble(strokeWidthAttr) is double sw)
                style.StrokeWidth = sw;

            var fillAttr = element.Attribute("fill")?.Value;
            if (!string.IsNullOrEmpty(fillAttr))
                style.FillColor = ParseColor(fillAttr);

            return style;
        }

        private void ApplyCADMetadata(ODElement element, XElement xmlElement)
        {
            // Extract CAD-specific attributes
            var cadType = xmlElement.Attribute(_cadNs + "type")?.Value;
            var cadLayer = xmlElement.Attribute(_cadNs + "layer-id")?.Value;
            var isBlockRef = xmlElement.Attribute(_cadNs + "blockref")?.Value == "true";

            // Store CAD metadata in your element
            // This depends on your ODElement implementation
            // You might want to add CADMetadata property to ODElement
        }

        #endregion

        #region Utility Methods (Original + Enhanced)

        private DateTime? ParseDateTime(string value)
        {
            if (string.IsNullOrEmpty(value)) return null;
            return DateTime.TryParse(value, System.Globalization.CultureInfo.InvariantCulture,
                System.Globalization.DateTimeStyles.None, out DateTime result) ? result : null;
        }

        private ODColour ParseColor(string colorValue)
        {
            if (string.IsNullOrEmpty(colorValue) || colorValue == "none")
                return ODColour.Transparent;

            // Handle rgb() format
            if (colorValue.StartsWith("rgb("))
            {
                var parts = colorValue.Substring(4, colorValue.Length - 5).Split(',');
                if (parts.Length == 3 &&
                    byte.TryParse(parts[0], out byte r) &&
                    byte.TryParse(parts[1], out byte g) &&
                    byte.TryParse(parts[2], out byte b))
                {
                    return new ODColour(r, g, b);
                }
            }

            // Handle hex colors
            if (colorValue.StartsWith("#"))
            {
                try
                {
                    return new ODColour(colorValue);
                }
                catch
                {
                    // Fall through to named colors
                }
            }

            // Handle named colors
            return colorValue.ToLower() switch
            {
                "black" => ODColour.Black,
                "red" => ODColour.Red,
                "green" => ODColour.Green,
                "blue" => ODColour.Blue,
                "white" => ODColour.White,
                _ => ODColour.Black // Default
            };
        }

        private string ParseLineType(string dashArray)
        {
            if (string.IsNullOrEmpty(dashArray) || dashArray == "none")
                return "Continuous";

            // Simple dash array parsing
            var parts = dashArray.Split(',');
            if (parts.Length >= 2 &&
                ParseDouble(parts[0]) is double dash &&
                ParseDouble(parts[1]) is double gap)
            {
                if (dash == 10 && gap == 5) return "Dashed";
                if (dash == 2 && gap == 5) return "Dotted";
                // Add more patterns as needed
            }

            return "Continuous";
        }

        private double? ParseDouble(string value)
        {
            if (string.IsNullOrEmpty(value)) return null;

            // Remove units if present
            value = value.Trim().ToLower();
            if (value.EndsWith("px")) value = value[..^2];
            else if (value.EndsWith("mm"))
            {
                value = value[..^2];
                if (double.TryParse(value, System.Globalization.NumberStyles.Any,
                    System.Globalization.CultureInfo.InvariantCulture, out double mm))
                    return mm * 3.779527559;
                return null;
            }
            // Add other unit conversions as needed

            if (double.TryParse(value, System.Globalization.NumberStyles.Any,
                System.Globalization.CultureInfo.InvariantCulture, out double result))
                return result;

            return null;
        }

        private double ParseDimension(string value)
        {
            if (string.IsNullOrEmpty(value)) return 0;

            // Remove units and parse
            value = value.Trim().ToLower();

            // Handle common units
            if (value.EndsWith("px")) value = value[..^2];
            else if (value.EndsWith("pt")) value = value[..^2];
            else if (value.EndsWith("mm"))
            {
                value = value[..^2];
                if (double.TryParse(value, System.Globalization.NumberStyles.Any,
                    System.Globalization.CultureInfo.InvariantCulture, out double mm))
                    return mm * 3.779527559; // Convert mm to pixels
            }
            else if (value.EndsWith("cm"))
            {
                value = value[..^2];
                if (double.TryParse(value, System.Globalization.NumberStyles.Any,
                    System.Globalization.CultureInfo.InvariantCulture, out double cm))
                    return cm * 37.795275591; // Convert cm to pixels
            }
            else if (value.EndsWith("in"))
            {
                value = value[..^2];
                if (double.TryParse(value, System.Globalization.NumberStyles.Any,
                    System.Globalization.CultureInfo.InvariantCulture, out double inches))
                    return inches * 96; // Convert inches to pixels
            }

            return double.TryParse(value, System.Globalization.NumberStyles.Any,
                System.Globalization.CultureInfo.InvariantCulture, out double result) ? result : 0;
        }

        private (double Width, double Height)? ParseViewBox(string viewBoxValue)
        {
            if (string.IsNullOrEmpty(viewBoxValue)) return null;

            var parts = viewBoxValue.Split(new[] { ' ', ',' }, StringSplitOptions.RemoveEmptyEntries);

            if (parts.Length >= 4 &&
                double.TryParse(parts[2], System.Globalization.NumberStyles.Any,
                    System.Globalization.CultureInfo.InvariantCulture, out double width) &&
                double.TryParse(parts[3], System.Globalization.NumberStyles.Any,
                    System.Globalization.CultureInfo.InvariantCulture, out double height))
            {
                return (width, height);
            }

            return null;
        }

        #endregion

        #region Properties and Public Methods

        public CADMetadata Metadata => _metadata;
        public IReadOnlyDictionary<string, CADLayer> Layers => _layers;
        public bool IsLoaded => _xDocument != null;

        public ODVec2 GetDimensions()
        {
            if (_xDocument == null)
                throw new InvalidOperationException("No SVG file loaded. Call LoadSvgFile first.");

            var svgElement = _xDocument.Root;
            var width = ParseDimension(svgElement.Attribute("width")?.Value);
            var height = ParseDimension(svgElement.Attribute("height")?.Value);

            // Fallback to viewBox if dimensions aren't found
            if (width == 0 || height == 0)
            {
                var viewBox = ParseViewBox(svgElement.Attribute("viewBox")?.Value);
                if (viewBox.HasValue)
                {
                    width = width == 0 ? viewBox.Value.Width : width;
                    height = height == 0 ? viewBox.Value.Height : height;
                }
            }

            Debug.WriteLine($"SVG Dimensions: Width={width}, Height={height}");
            return new ODVec2(width, height);
        }

        public void Unload()
        {
            _xDocument = null;
            _filePath = null;
            _layers.Clear();
            _blockDefinitions.Clear();
            _metadata = new CADMetadata();
        }

        #endregion
    }

    public class CADMetadata
    {
        public string? AppVersion { get; set; }
        public string? FileVersion { get; set; }
        public string? Units { get; set; }
        public double Precision { get; set; }
        public string? Application { get; set; }
        public string? Version { get; set; }
        public DateTime? Created { get; set; }
        public DateTime? Modified { get; set; }
        public string? Title { get; set; }
        public string? Creator { get; set; }
        public string? Description { get; set; }
        public Dictionary<string, string>? Properties { get; set; }
    }
}
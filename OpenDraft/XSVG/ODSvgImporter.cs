using Avalonia.Controls.Shapes;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text;
using System.Xml.Linq;

namespace OpenDraft.XSVG
{
    internal struct SVGStyle
    {
        public ODColour StrokeColor;
        public float StrokeWidth;
        public string LineType;
    }

    public class SvgImporter
    {
        private string? _filePath;
        private XDocument? _xDocument;
        private XNamespace _ns = "http://www.w3.org/2000/svg";

        public void LoadSvgFile(string filePath)
        {
            _filePath = filePath;

            if (!System.IO.File.Exists(filePath))
                throw new System.IO.FileNotFoundException("SVG file not found", filePath);
            
            string fileContentsRaw = System.IO.File.ReadAllText(filePath);

            _xDocument = XDocument.Parse(fileContentsRaw);
        }


        // Add dimension extraction methods
        public ODPoint GetDimensions()
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
            return new ODPoint(width, height);
        }


        // Utility methods
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

        public List<ODElement> ImportGeometry()
        {
            if (_xDocument == null)
                throw new InvalidOperationException("No SVG file loaded. Call LoadSvgFile first.");

            var geometries = new List<ODElement>();
            var svgElement = _xDocument.Root;

            // Import paths (most common CAD element)
            geometries.AddRange(ImportPaths(svgElement));

            // Import basic shapes
            geometries.AddRange(ImportRectangles(svgElement));
            geometries.AddRange(ImportLines(svgElement));
            geometries.AddRange(ImportCircles(svgElement));

            Debug.WriteLine($"Imported {geometries.Count} geometry objects");
            return geometries;
        }

        private List<ODElement> ImportPaths(XElement svgElement)
        {
            var paths = new List<ODElement>();

            foreach (var pathElement in svgElement.Descendants(_ns + "path"))
            {
                var pathData = pathElement.Attribute("d")?.Value;
                if (!string.IsNullOrEmpty(pathData))
                {
                    var style = ParseStyle(pathElement);
                    var path = ParsePathData(pathData, style);
                    if (path != null)
                    {
                        paths.Add(path);
                    }
                }
            }

            return paths;
        }

        private List<ODElement> ImportRectangles(XElement svgElement)
        {
            var rectangles = new List<ODElement>();

            foreach (var rectElement in svgElement.Descendants(_ns + "rect"))
            {
                var x = ParseDouble(rectElement.Attribute("x")?.Value) ?? 0;
                var y = ParseDouble(rectElement.Attribute("y")?.Value) ?? 0;
                var width = ParseDouble(rectElement.Attribute("width")?.Value) ?? 0;
                var height = ParseDouble(rectElement.Attribute("height")?.Value) ?? 0;

                if (width > 0 && height > 0)
                {
                    var style = ParseStyle(rectElement);
                    var rectangle = new ODRectangle(
                        new ODPoint(x, y),
                        new ODPoint(x + width, y + height)
                    );
                    rectangles.Add(rectangle);
                }
            }

            return rectangles;
        }

        private List<ODElement> ImportLines(XElement svgElement)
        {
            var lines = new List<ODElement>();

            foreach (var lineElement in svgElement.Descendants(_ns + "line"))
            {
                var x1 = ParseDouble(lineElement.Attribute("x1")?.Value) ?? 0;
                var y1 = ParseDouble(lineElement.Attribute("y1")?.Value) ?? 0;
                var x2 = ParseDouble(lineElement.Attribute("x2")?.Value) ?? 0;
                var y2 = ParseDouble(lineElement.Attribute("y2")?.Value) ?? 0;

                var style = ParseStyle(lineElement);
                var line = new ODLine(
                    new ODPoint(x1, y1),
                    new ODPoint(x2, y2)
                    //style <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< todo?????
                );
                lines.Add(line);
            }

            return lines;
        }

        private List<ODElement> ImportCircles(XElement svgElement)
        {
            var circles = new List<ODElement>();

            foreach (var circleElement in svgElement.Descendants(_ns + "circle"))
            {
                var cx = ParseDouble(circleElement.Attribute("cx")?.Value) ?? 0;
                var cy = ParseDouble(circleElement.Attribute("cy")?.Value) ?? 0;
                var r = ParseDouble(circleElement.Attribute("r")?.Value) ?? 0;

                if (r > 0)
                {
                    var style = ParseStyle(circleElement);
                    var circle = new ODCircle(
                        new ODPoint(cx, cy),
                        r
                        //style <<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<<< todo?
                    );
                    circles.Add(circle);
                }
            }

            return circles;
        }

        private ODElement ParsePathData(string pathData, SVGStyle style)
        {
            try
            {
                var points = new List<ODPoint>();
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
                                points.Add(new ODPoint(currentX, currentY));
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
                                points.Add(new ODPoint(currentX, currentY));
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
                                points.Add(new ODPoint(currentX, currentY));
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
                                points.Add(new ODPoint(currentX, currentY));
                                i += 1;
                                hasValidData = true;
                            }
                            break;

                        case "Z": // Close Path
                            if (points.Count > 0)
                            {
                                // Close the path by returning to start point
                                points.Add(new ODPoint(startX, startY));
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
        private SVGStyle ParseStyle(XElement element)
        {
            var style = new SVGStyle();

            // Parse style attribute
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
                                    style.StrokeWidth = (float)strokeWidth;
                                break;
                            case "stroke-dasharray":
                                style.LineType = ParseLineType(value);
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
                style.StrokeWidth = (float)sw;

            return style;
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









        // Cleanup method
        public void Unload()
        {
            _xDocument = null;
            _filePath = null;
        }

        // Property to check if file is loaded
        public bool IsLoaded => _xDocument != null;
    }

    
}

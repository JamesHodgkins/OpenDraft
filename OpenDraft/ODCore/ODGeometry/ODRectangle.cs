using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Xml.Linq;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODRectangle : ODElement
    {
        public ODPoint TopLeft { get; set; }
        public ODPoint BottomRight { get; set; }

        public ODRectangle(ODPoint topLeft, ODPoint bottomRight)
        {
            TopLeft = topLeft;
            BottomRight = bottomRight;
        }

        // Alternative constructor from center point and dimensions
        public ODRectangle(ODPoint center, double width, double height)
        {
            var halfWidth = width / 2;
            var halfHeight = height / 2;
            TopLeft = new ODPoint(center.X - halfWidth, center.Y - halfHeight);
            BottomRight = new ODPoint(center.X + halfWidth, center.Y + halfHeight);
        }

        // Alternative constructor from corner point and dimensions
        public ODRectangle(ODPoint corner, double width, double height, bool isTopLeft = true)
        {
            if (isTopLeft)
            {
                TopLeft = corner;
                BottomRight = new ODPoint(corner.X + width, corner.Y + height);
            }
            else
            {
                BottomRight = corner;
                TopLeft = new ODPoint(corner.X - width, corner.Y - height);
            }
        }

        public override void Draw(DrawingContext context, ODDataManager dm)
        {
            ODLayer? layer = dm.LayerManager.GetLayerByID(LayerId);
            ODLineStyleRegistry lsRegistry = dm.LayerManager.LineStyleRegistry;

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            // Get dash style from registry
            IDashStyle dashStyle = lsRegistry.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            string effectiveColour = (Colour != null) ? Colour.ToHex() : layer.Color;
            float effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen for the outline
            Pen pen = new Pen(
                new SolidColorBrush(Color.Parse(effectiveColour)), // Brush only
                effectiveLineWeight,                               // Thickness
                dashStyle                                          // Dash style
            );

            // Create rectangle geometry
            Rect rect = new Rect(
                new Point(TopLeft.X, TopLeft.Y),
                new Point(BottomRight.X, BottomRight.Y)
            );

            // Draw the rectangle
            context.DrawRectangle(pen, rect);
        }

        // Properties for convenience
        public double X => TopLeft.X;
        public double Y => TopLeft.Y;
        public double Width => Math.Abs(BottomRight.X - TopLeft.X);
        public double Height => Math.Abs(BottomRight.Y - TopLeft.Y);

        public ODPoint TopRight => new ODPoint(BottomRight.X, TopLeft.Y);
        public ODPoint BottomLeft => new ODPoint(TopLeft.X, BottomRight.Y);
        public ODPoint Center => new ODPoint(
            (TopLeft.X + BottomRight.X) / 2,
            (TopLeft.Y + BottomRight.Y) / 2
        );

        public double Area => Width * Height;
        public double Perimeter => 2 * (Width + Height);

        // Method to check if a point is inside the rectangle
        public bool ContainsPoint(ODPoint point)
        {
            var minX = Math.Min(TopLeft.X, BottomRight.X);
            var maxX = Math.Max(TopLeft.X, BottomRight.X);
            var minY = Math.Min(TopLeft.Y, BottomRight.Y);
            var maxY = Math.Max(TopLeft.Y, BottomRight.Y);

            return point.X >= minX && point.X <= maxX &&
                   point.Y >= minY && point.Y <= maxY;
        }


        // Convert to polyline (useful for some operations)
        public ODPolyline ToPolyline()
        {
            var points = new List<ODPoint>
            {
                TopLeft,
                TopRight,
                BottomRight,
                BottomLeft,
                TopLeft // Close the rectangle
            };
            return new ODPolyline(points);
        }

        // Get bounding box (for compatibility with other geometry)
        public ODRectangle GetBoundingBox()
        {
            return this; // A rectangle is its own bounding box
        }

        // SVG export support
        public XElement ToSvgElement(ODLayer layer, ODLineStyleRegistry registry)
        {
            var element = new XElement("{http://www.w3.org/2000/svg}rect",
                new XAttribute("x", TopLeft.X),
                new XAttribute("y", TopLeft.Y),
                new XAttribute("width", Width),
                new XAttribute("height", Height),
                new XAttribute("fill", "none") // No fill by default
            );

            return element;
        }

        public override string ToString()
        {
            return $"Rectangle [({X:0.##}, {Y:0.##}) {Width:0.##}x{Height:0.##}]";
        }
    }
}
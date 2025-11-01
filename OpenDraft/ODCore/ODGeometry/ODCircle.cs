using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODMath;
using System;
using System.Net;

namespace OpenDraft.ODCore.ODGeometry
{
    internal class ODCircle : ODElement
    {
        public ODVec2 Center { get; set; }
        public double Radius { get; set; }

        public ODCircle(ODVec2 center, double radius)
        {
            Center = center;
            Radius = radius;
        }

        public override void Draw(DrawingContext context, ODDataService connector)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            // Get dash style from registry
            var dashStyle = connector.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            ODColour effectiveColour = (Colour != null) ? Colour : layer.Color;
            double effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            var pen = new Pen(
                new SolidColorBrush(effectiveColour.ToAvaloniaColor()), // Brush only
                effectiveLineWeight,                                    // Thickness
                dashStyle                                               // Dash style
            );

            context.DrawEllipse(null, pen, new Point(Center.X, Center.Y), Radius, Radius);

        }


        public override void DrawHighlight(DrawingContext context, ODDataService connector, ODColour hColour, int hIntensity)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer != null && !layer.IsVisible)
                return;

            // Create pen
            double effectiveLineWeight = LineWeight ?? layer!.LineWeight;

            var pen = new Pen(new SolidColorBrush(Color.FromArgb((byte)hIntensity, hColour.R, hColour.G, hColour.B)), effectiveLineWeight + 1);

            context.DrawEllipse(null, pen, new Point(Center.X, Center.Y), Radius, Radius);
        }


        public override ODBoundingBox GetBoundingBox()
        {
            double minX = Center.X - Radius;
            double minY = Center.Y - Radius;
            double maxX = Center.X + Radius;
            double maxY = Center.Y + Radius;

            return ODBoundingBox.CreateFromMinMax(new ODVec2(minX, minY), new ODVec2(maxX, maxY));
        }

        public override bool HitTest(ODVec2 point, double tolerance)
        {
            if (!isPointInsideBoundingBox(point))
                return false;

            if (ODGeometryTools.IsPointOnCircle(point, Center, Radius, tolerance))
                return true;

            return false;
        }
    }
}

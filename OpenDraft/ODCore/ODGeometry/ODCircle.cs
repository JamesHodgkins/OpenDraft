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

        public override void Draw(DrawingContext context, ODDrawConnector connector)
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



            var bbpen = new Pen(new SolidColorBrush(Colors.Yellow, 1));
            ODBoundingBox bb = GetBoundingBox();
            var rect = new Rect(bb.GetOrigin.X,
                                bb.GetOrigin.Y,
                                bb.Width,
                                bb.Height);

            context.DrawRectangle(bbpen, rect);
        }


        public override ODBoundingBox GetBoundingBox()
        {
            double minX = Center.X - Radius;
            double minY = Center.Y - Radius;
            double maxX = Center.X + Radius;
            double maxY = Center.Y + Radius;

            return ODBoundingBox.CreateFromMinMax(new ODVec2(minX, minY), new ODVec2(maxX, maxY));
        }

        public override bool HitTest(ODVec2 point)
        {
            return false;
        }
    }
}

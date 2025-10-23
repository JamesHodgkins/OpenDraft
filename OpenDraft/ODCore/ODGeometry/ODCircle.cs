using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODMath;

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
        }
    }
}

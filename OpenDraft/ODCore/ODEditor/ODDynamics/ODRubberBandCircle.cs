using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;
using System.Diagnostics;
using System.Net;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODRubberBandCircle : ODDynamicElement
    {
        public ODVec2 Start { get; set; }
        public float Weight { get; set; } = 1.0f;

        public ODRubberBandCircle(ODVec2 start)
        {
            Start = start;
        }

        public override void Draw(DrawingContext context, ODDrawConnector connector,
            double scale, ODVec2 vpWorldSize, ODVec2 mousePosition)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer == null || !layer.IsVisible || layer.Color == null)
                return;

            // Get dash style from registry
            IDashStyle dashStyle = connector.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            string effectiveColour = (Colour != null) ? Colour.ToHex() : layer.Color.ToHex();
            double effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            Pen pen = new Pen(
                new SolidColorBrush(Color.Parse(effectiveColour)), // Brush only
                effectiveLineWeight,                               // Thickness
                dashStyle                                          // Dash style
            );

            double radius = Math.Sqrt(
                Math.Pow(mousePosition.X - Start.X, 2) + Math.Pow(mousePosition.Y - Start.Y, 2)
                );

            context.DrawEllipse(null, pen,
                new Point(Start.X, Start.Y),
                Math.Abs(radius),
                Math.Abs(radius));
        }
    }
}

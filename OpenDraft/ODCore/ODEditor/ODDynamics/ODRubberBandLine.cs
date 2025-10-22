using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODRubberBandLine : ODDynamicElement
    {
        public ODPoint Start { get; set; }

        public ODRubberBandLine(ODPoint start)
        {
            Start = start;
        }

        public override void Draw(DrawingContext context, ODDrawConnector connector,
            float scale, ODPoint vpWorldSize, ODPoint mousePosition)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer == null || !layer.IsVisible || layer.Color == null)
                return;

            // Get dash style from registry
            IDashStyle dashStyle = connector.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            string effectiveColour = (Colour != null) ? Colour.ToHex() : layer.Color.ToHex();
            float effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            Pen pen = new Pen(
                new SolidColorBrush(Color.Parse(effectiveColour)), // Brush only
                effectiveLineWeight,                               // Thickness
                dashStyle                                          // Dash style
            );

            context.DrawLine(pen, new Point(Start.X, Start.Y), new Point(mousePosition.X, mousePosition.Y));
        }
    }
}

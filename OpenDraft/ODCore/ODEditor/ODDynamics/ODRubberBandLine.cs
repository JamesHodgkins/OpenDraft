using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODRubberBandLine : ODDynamicElement
    {
        public ODVec2 Start { get; set; }

        public ODRubberBandLine(ODVec2 start)
        {
            Start = start;
        }

        public override void Draw(DrawingContext context, ODDataService connector,
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

            context.DrawLine(pen, new Point(Start.X, Start.Y), new Point(mousePosition.X, mousePosition.Y));
        }
    }
}

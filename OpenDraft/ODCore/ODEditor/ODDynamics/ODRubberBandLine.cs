using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Diagnostics;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODRubberBandLine : ODDynamicElement
    {
        public ODPoint Start { get; set; }
        public float Weight { get; set; } = 1.0f;

        public ODRubberBandLine(ODPoint start)
        {
            Start = start;
        }

        public override void Draw(DrawingContext context, ODLayer layer, ODLineStyleRegistry lsRegistry,
            float scale, ODPoint vpWorldSize, ODPoint mousePosition)
        {
            if (layer == null || !layer.IsVisible || layer.Color == null)
                return;

            Pen pen = new Pen(new SolidColorBrush(Avalonia.Media.Color.Parse(layer.Color)), 
                layer.LineWeight, lsRegistry.ToAvaloniaDashStyle(layer.LineType));

            context.DrawLine(pen, new Point(Start.X, Start.Y), new Point(mousePosition.X, mousePosition.Y));
        }
    }
}

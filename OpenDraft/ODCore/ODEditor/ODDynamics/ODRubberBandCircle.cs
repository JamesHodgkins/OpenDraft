using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Diagnostics;
using System.Net;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODRubberBandCircle : ODDynamicElement
    {
        public ODPoint Start { get; set; }
        public float Weight { get; set; } = 1.0f;

        public ODRubberBandCircle(ODPoint start)
        {
            Start = start;
        }

        public override void Draw(DrawingContext context, ODDrawConnector connector,
            float scale, ODPoint vpWorldSize, ODPoint mousePosition)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer == null || !layer.IsVisible || layer.Color == null)
                return;

            Pen pen = new Pen(new SolidColorBrush(Avalonia.Media.Color.Parse(layer.Color)), 
                layer.LineWeight, connector.ToAvaloniaDashStyle(layer.LineType));

            float radius = (float)Math.Sqrt(
                Math.Pow(mousePosition.X - Start.X, 2) + Math.Pow(mousePosition.Y - Start.Y, 2)
                );

            context.DrawEllipse(null, pen,
                new Point(Start.X, Start.Y),
                Math.Abs(radius),
                Math.Abs(radius));
        }
    }
}

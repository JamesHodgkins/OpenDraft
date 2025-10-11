using Avalonia;
using Avalonia.Controls.Shapes;
using Avalonia.Media;
using OpenDraft.Core.ODData;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.Core.ODGeometry
{
    internal class ODLine : ODElement
    {
        public ODPoint StartPoint { get; set; }
        public ODPoint EndPoint { get; set; }


        public ODLine(ODPoint startPoint, ODPoint endPoint)
        {
            StartPoint = startPoint;
            EndPoint = endPoint;
        }


        public override void Draw(DrawingContext context, ODLayer layer)
        {
            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;
            
            Pen pen = new Pen(new SolidColorBrush(Color.Parse(layer.Color)), layer.LineWeight);

            context.DrawLine(pen,
                    new Point(StartPoint.X, StartPoint.Y),
                    new Point(EndPoint.X, EndPoint.Y));
        }

    }
}

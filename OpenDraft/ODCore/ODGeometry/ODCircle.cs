using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODGeometry
{
    internal class ODCircle : ODElement
    {
        public ODPoint Center { get; set; }
        public double Radius { get; set; }

        public ODCircle(ODPoint center, double radius)
        {
            Center = center;
            Radius = radius;
        }

        public override void Draw(DrawingContext context, ODLayer layer)
        {
            context.DrawEllipse(
                Brushes.Red, new Pen(Brushes.Red, 0.1),
                new Point(50, 50),
                0.4, 0.4
            );
        }
    }
}

using Avalonia;
using Avalonia.Media;
using OpenDraft.Core.ODData;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.Core.ODGeometry
{
    internal class ODCircle : ODElement
    {


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

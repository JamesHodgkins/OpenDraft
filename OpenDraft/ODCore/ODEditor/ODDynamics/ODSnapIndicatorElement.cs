using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class SnapIndicatorElement : ODDynamicElement
    {
        public ODPoint Position { get; set; }
        public double Radius { get; set; } = 5.0;

        public SnapIndicatorElement(ODPoint position)
        {
            Position = position;
        }

        public override void Draw(DrawingContext context, ODDataManager dm, 
            float scale, ODPoint vpExtents, ODPoint worldMousePosition)
        {
            var brush = new SolidColorBrush(Colors.Yellow);
            var pen = new Pen(brush, 1.5);

            context.DrawEllipse(null, pen,
                new Point(Position.X, Position.Y),
                Radius, Radius);
        }
    }
}

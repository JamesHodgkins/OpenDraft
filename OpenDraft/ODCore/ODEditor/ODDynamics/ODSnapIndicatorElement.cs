using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class SnapIndicatorElement : ODDynamicElement
    {
        public ODVec2 Position { get; set; }
        public double Radius { get; set; } = 5.0;

        public SnapIndicatorElement(ODVec2 position)
        {
            Position = position;
        }

        public override void Draw(DrawingContext context, ODDrawConnector connector, 
            double scale, ODVec2 vpExtents, ODVec2 worldMousePosition)
        {
            var brush = new SolidColorBrush(Colors.Yellow);
            var pen = new Pen(brush, 1.5);

            context.DrawEllipse(null, pen,
                new Point(Position.X, Position.Y),
                Radius, Radius);
        }
    }
}

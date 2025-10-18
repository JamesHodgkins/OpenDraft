using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    // Example dynamic element classes
    public class ODCrosshairElement : ODDynamicElement
    {
        public ODPoint Center { get; set; }
        public double Size { get; set; } = 20.0;

        public ODCrosshairElement(ODPoint center)
        {
            Center = center;
        }

        public override void Draw(DrawingContext context)
        {
            var pen = new Pen(Brushes.Red, 2.0);

            // Horizontal line
            context.DrawLine(pen,
                new Point(Center.X - Size, Center.Y),
                new Point(Center.X + Size, Center.Y));

            // Vertical line  
            context.DrawLine(pen,
                new Point(Center.X, Center.Y - Size),
                new Point(Center.X, Center.Y + Size));
        }
    }
}

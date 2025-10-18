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
        private bool isVisible = true;
        private int size = 10;
        private Color hColor = Colors.Red;
        private Color vColor = Colors.Lime;
        private Color cColor = Colors.White;


        SolidColorBrush hBrush;
        SolidColorBrush vBrush;
        SolidColorBrush cBrush;

        Pen hPen;
        Pen vPen;
        Pen cPen;



        public ODCrosshairElement(ODPoint center)
        {
            Center = center;

            hBrush = new SolidColorBrush(hColor);
            vBrush = new SolidColorBrush(vColor);
            cBrush = new SolidColorBrush(cColor);

            hPen = new Pen(hBrush);
            vPen = new Pen(vBrush);
            cPen = new Pen(cBrush);
        }

        public void show() => isVisible = true;
        public void hide() => isVisible = false;

        public override void Draw(DrawingContext context, float scale, ODPoint vpWorldSize)
        {
            if (!isVisible)
                return;

            float vpWidth = vpWorldSize.X;
            float vpHeight = vpWorldSize.Y;
            float thickness = 1.0f / scale;

            hPen.Thickness = thickness;
            vPen.Thickness = thickness;
            cPen.Thickness = thickness;

            float cSize = size / scale;

            // Centre square
            context.DrawRectangle(null, cPen,
                new Rect(
                    new Point(Center.X - cSize / 2, Center.Y - cSize / 2),
                    new Size(cSize, cSize)
                ));


            // Vertical line (Top)
            context.DrawLine(vPen,
                new Point(Center.X, Center.Y + vpHeight),
                new Point(Center.X, Center.Y + cSize/2));
            
            // Horizontal line (Left)
            context.DrawLine(hPen,
                new Point(Center.X - vpWidth, Center.Y),
                new Point(Center.X - cSize/2, Center.Y));

            // Vertical line (Bottom)
            context.DrawLine(vPen,
                new Point(Center.X, Center.Y - cSize/2),
                new Point(Center.X, Center.Y - vpHeight));

            // Horizontal line (Right)
            context.DrawLine(hPen,
                new Point(Center.X + cSize/2, Center.Y),
                new Point(Center.X + vpWidth, Center.Y));

        }
    }
}

using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODSystem;
using OpenDraft.ODCore.ODMath;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODCrosshairElement : ODDynamicElement
    {
        // Visibility
        public bool IsVisible { get; set; } = true;

        // Style properties
        private int _size = 10;
        public int Size
        {
            get => _size;
            set => _size = value;
        }

        public ODCrosshairElement()
        {

        }

        public override void Draw(DrawingContext context, ODDrawConnector connector,
            double scale, ODVec2 vpWorldSize, ODVec2 worldMousePosition)
        {
            if (!IsVisible)
                return;

            ODLayer? layer = connector.GetLayerByID(LayerId);

            // Get layer
            ODVec2 Center = worldMousePosition;
            double cSize = Size / scale; // Centre square size

            // Get styles from registry
            Color xColour = Avalonia.Media.Color.Parse(ODSystem.ODSystem.GetRegistryValueAsString("style/crosshair_x_colour") ?? "Red");
            Color yColour = Avalonia.Media.Color.Parse(ODSystem.ODSystem.GetRegistryValueAsString("style/crosshair_y_colour") ?? "Lime");
            Color sqColour = Avalonia.Media.Color.Parse(ODSystem.ODSystem.GetRegistryValueAsString("style/crosshair_sq_colour") ?? "White");
            float xThickness = ODSystem.ODSystem.GetRegistryValueAsDecimal("style/crosshair_line_weight") ?? 1;

            Pen xPen = new Pen(new SolidColorBrush(xColour), xThickness / scale);
            Pen yPen = new Pen(new SolidColorBrush(yColour), xThickness / scale);
            Pen sqPen = new Pen(new SolidColorBrush(sqColour), xThickness / scale);


            // Centre square
            context.DrawRectangle(null, sqPen,
                new Rect(
                    new Point(Center.X - cSize / 2, Center.Y - cSize / 2),
                    new Size(cSize, cSize)
                ));

            var top = new Point(Center.X, Center.Y + vpWorldSize.Y);
            var bottom = new Point(Center.X, Center.Y - vpWorldSize.Y);
            var left = new Point(Center.X - vpWorldSize.X, Center.Y);
            var right = new Point(Center.X + vpWorldSize.X, Center.Y);

            context.DrawLine(yPen, top, new Point(Center.X, Center.Y + cSize / 2)); // Top
            context.DrawLine(xPen, left, new Point(Center.X - cSize / 2, Center.Y)); // Left
            context.DrawLine(yPen, new Point(Center.X, Center.Y - cSize / 2), bottom); // Bottom
            context.DrawLine(xPen, new Point(Center.X + cSize / 2, Center.Y), right); // Right
        }

    }
}

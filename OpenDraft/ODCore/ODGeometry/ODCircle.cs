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

        public override void Draw(DrawingContext context, ODLayerManager lm)
        {
            ODLayer? layer = lm.GetLayerByID(LayerId);
            ODLineStyleRegistry lsRegistry = lm.LineStyleRegistry;

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            // Get dash style from registry
            var dashStyle = lsRegistry.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            string effectiveColour = (Colour != null) ? Colour.ToHex() : layer.Color;
            float effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            var pen = new Pen(
                new SolidColorBrush(Color.Parse(effectiveColour)), // Brush only
                effectiveLineWeight,                               // Thickness
                dashStyle                                          // Dash style
            );

            context.DrawEllipse(null, pen, new Point(Center.X, Center.Y), Radius, Radius);
        }
    }
}

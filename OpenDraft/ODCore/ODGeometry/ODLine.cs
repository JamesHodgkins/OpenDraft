using Avalonia;
using Avalonia.Media;
using Microsoft.Win32;
using OpenDraft.ODCore.ODData;
using System;

namespace OpenDraft.ODCore.ODGeometry
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


        public override void Draw(DrawingContext context, ODDrawConnector connector)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            // Get dash style from registry
            var dashStyle = connector.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            string effectiveColour = (Colour != null) ? Colour.ToHex() : layer.Color;
            float effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            Pen pen = new Pen(
                new SolidColorBrush(Color.Parse(effectiveColour)), // Brush only
                effectiveLineWeight,                               // Thickness
                dashStyle                                          // Dash style
            );

            context.DrawLine(pen,
                    new Point(StartPoint.X, StartPoint.Y),
                    new Point(EndPoint.X, EndPoint.Y));
        }

    }
}

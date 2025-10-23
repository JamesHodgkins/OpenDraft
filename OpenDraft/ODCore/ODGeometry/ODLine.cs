using Avalonia;
using Avalonia.Media;
using Microsoft.Win32;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODMath;
using System;

namespace OpenDraft.ODCore.ODGeometry
{
    internal class ODLine : ODElement
    {
        public ODVec2 StartPoint { get; set; }
        public ODVec2 EndPoint { get; set; }


        public ODLine(ODVec2 startPoint, ODVec2 endPoint)
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
            IDashStyle dashStyle = connector.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            ODColour effectiveColour = (Colour != null) ? Colour : layer.Color;
            double effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            var pen = new Pen(
                new SolidColorBrush(effectiveColour.ToAvaloniaColor()), // Brush only
                effectiveLineWeight,                                    // Thickness
                dashStyle                                               // Dash style
            );

            context.DrawLine(pen,
                    new Point(StartPoint.X, StartPoint.Y),
                    new Point(EndPoint.X, EndPoint.Y));
        }

    }
}

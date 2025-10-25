using Avalonia;
using Avalonia.Media;
using Microsoft.Win32;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODMath;
using System;
using System.Diagnostics;

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


        public override void DrawHighlight(DrawingContext context, ODDrawConnector connector, ODColour hColour, int hIntensity)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer != null && !layer.IsVisible)
                return;

            // Create pen
            double effectiveLineWeight = LineWeight ?? layer!.LineWeight;

            var pen = new Pen(new SolidColorBrush(Color.FromArgb((byte)hIntensity, hColour.R, hColour.G, hColour.B)), effectiveLineWeight+1);            

            context.DrawLine(pen,
                    new Point(StartPoint.X, StartPoint.Y),
                    new Point(EndPoint.X, EndPoint.Y));
        }


        public override ODBoundingBox GetBoundingBox()
        {
            double minX = Math.Min(StartPoint.X, EndPoint.X);
            double minY = Math.Min(StartPoint.Y, EndPoint.Y);
            double maxX = Math.Max(StartPoint.X, EndPoint.X);
            double maxY = Math.Max(StartPoint.Y, EndPoint.Y);

            return ODBoundingBox.CreateFromMinMax(new ODVec2(minX, minY), new ODVec2(maxX, maxY));
        }


        public override bool HitTest(ODVec2 point, double tolerance)
        {
            if (!isPointInsideBoundingBox(point))
                return false;

            if (GeometryTools.IsPointOnLine(point, StartPoint, EndPoint, tolerance))
                return true;

            return false;
        }

    }
}

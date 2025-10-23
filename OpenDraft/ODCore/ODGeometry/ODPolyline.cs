using Avalonia;
using Avalonia.Media;
using Microsoft.Win32;
using OpenDraft.ODCore.ODData;
using System;
using System.Collections.Generic;
using OpenDraft.ODCore.ODMath;
using System.Linq;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODPolyline : ODElement
    {
        public List<ODVec2> Points { get; set; }

        public ODPolyline(List<ODVec2> points)
        {
            Points = points ?? new List<ODVec2>();
        }

        public override void Draw(DrawingContext context, ODDrawConnector connector)
        {
            ODLayer? layer = connector.GetLayerByID(LayerId);

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            if (Points == null || Points.Count < 2)
                return;

            // Get dash style from registry
            var dashStyle = connector.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            ODColour effectiveColour = (Colour != null) ? Colour : layer.Color;
            double effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen
            var pen = new Pen(
                new SolidColorBrush(effectiveColour.ToAvaloniaColor()), // Brush only
                effectiveLineWeight,                                    // Thickness
                dashStyle                                               // Dash style
            );

            // Draw each segment individually
            for (int i = 1; i < Points.Count; i++)
            {
                context.DrawLine(pen,
                    new Point(Points[i - 1].X, Points[i - 1].Y),
                    new Point(Points[i].X, Points[i].Y));
            }
        }

        // Optional: Add convenience properties and methods
        public bool IsClosed => Points.Count > 2 && Points[0].Equals(Points[Points.Count - 1]);

        public double Length
        {
            get
            {
                if (Points.Count < 2) return 0;

                double length = 0;
                for (int i = 1; i < Points.Count; i++)
                {
                    length += Math.Sqrt(
                        Math.Pow(Points[i].X - Points[i - 1].X, 2) +
                        Math.Pow(Points[i].Y - Points[i - 1].Y, 2));
                }
                return length;
            }
        }

        public ODVec2 StartPoint => Points?.FirstOrDefault() ?? new ODVec2(0, 0);
        public ODVec2 EndPoint => Points?.LastOrDefault() ?? new ODVec2(0, 0);

        public void AddPoint(ODVec2 point)
        {
            Points.Add(point);
        }

        public void InsertPoint(int index, ODVec2 point)
        {
            if (index >= 0 && index <= Points.Count)
                Points.Insert(index, point);
        }

        public void RemovePoint(int index)
        {
            if (index >= 0 && index < Points.Count)
                Points.RemoveAt(index);
        }

        public ODRectangle GetBoundingBox()
        {
            if (Points == null || Points.Count == 0)
                return new ODRectangle(new ODVec2(0, 0), new ODVec2(0, 0));

            double minX = Points.Min(p => p.X);
            double maxX = Points.Max(p => p.X);
            double minY = Points.Min(p => p.Y);
            double maxY = Points.Max(p => p.Y);

            return new ODRectangle(new ODVec2(minX, minY), new ODVec2(maxX, maxY));
        }
    }
}
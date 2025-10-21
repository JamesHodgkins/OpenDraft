using Avalonia;
using Avalonia.Media;
using Microsoft.Win32;
using OpenDraft.ODCore.ODData;
using System;
using System.Collections.Generic;
using System.Linq;

namespace OpenDraft.ODCore.ODGeometry
{
    public class ODPolyline : ODElement
    {
        public List<ODPoint> Points { get; set; }

        public ODPolyline(List<ODPoint> points)
        {
            Points = points ?? new List<ODPoint>();
        }

        public override void Draw(DrawingContext context, ODLayerManager lm)
        {
            ODLayer? layer = lm.GetLayerByID(LayerId);
            ODLineStyleRegistry lsRegistry = lm.LineStyleRegistry;

            if (layer != null && !layer.IsVisible)
                return;

            if (layer == null)
                return;

            if (Points == null || Points.Count < 2)
                return;

            // Get dash style from registry
            var dashStyle = lsRegistry.ToAvaloniaDashStyle(LineType ?? layer.LineType);
            string effectiveColour = (Colour != null) ? Colour.ToHex() : layer.Color;
            float effectiveLineWeight = LineWeight ?? layer.LineWeight;

            // Create pen for the outline
            var pen = new Pen(
                new SolidColorBrush(Color.Parse(effectiveColour)), // Brush only
                effectiveLineWeight,                               // Thickness
                dashStyle                                          // Dash style
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

        public ODPoint StartPoint => Points?.FirstOrDefault() ?? new ODPoint(0, 0);
        public ODPoint EndPoint => Points?.LastOrDefault() ?? new ODPoint(0, 0);

        public void AddPoint(ODPoint point)
        {
            Points.Add(point);
        }

        public void InsertPoint(int index, ODPoint point)
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
                return new ODRectangle(new ODPoint(0, 0), new ODPoint(0, 0));

            double minX = Points.Min(p => p.X);
            double maxX = Points.Max(p => p.X);
            double minY = Points.Min(p => p.Y);
            double maxY = Points.Max(p => p.Y);

            return new ODRectangle(new ODPoint(minX, minY), new ODPoint(maxX, maxY));
        }
    }
}
using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Diagnostics;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODRubberBandLine : ODDynamicElement
    {
        public ODPoint Start { get; set; }
        public float Weight { get; set; } = 1.0f;

        // Visibility
        public bool IsVisible { get; set; } = true;

        private float _thickness = 2.0f;
        public float Thickness
        {
            get => _thickness;
            set
            {
                _thickness = value;
                UpdatePens();
            }
        }

        private Color _hColor = Colors.Gray;
        public Color Colour
        {
            get => _hColor;
            set
            {
                _hColor = value;
                UpdatePens();
            }
        }

        private SolidColorBrush brush;
        private Pen pen;

        public ODRubberBandLine(ODPoint start)
        {
            Start = start;
            UpdatePens();
        }

        private void UpdatePens()
        {
            brush = new SolidColorBrush(_hColor);
            pen = new Pen(brush, _thickness, DashStyle.Dash);
        }

        public void SetColour(string sColour)
        {
            Colour = Avalonia.Media.Color.Parse(sColour);
        }

        public override void Draw(DrawingContext context, float scale, ODPoint vpWorldSize, ODPoint mousePosition)
        {
            if (!IsVisible)
                return;

            _thickness = Weight / scale;
            pen.Thickness = _thickness;

            context.DrawLine(pen, new Point(Start.X, Start.Y), new Point(mousePosition.X, mousePosition.Y));
        }

    }
}

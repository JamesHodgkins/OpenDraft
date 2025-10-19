using Avalonia;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;

namespace OpenDraft.ODCore.ODEditor.ODDynamics
{
    public class ODCrosshairElement : ODDynamicElement
    {
        public ODPoint Center { get; set; }
        public float Weight { get; set; } = 1.0f;

        // Visibility
        public bool IsVisible { get; set; } = true;

        // Style properties
        private int _size = 10;
        public int Size
        {
            get => _size;
            set => _size = value;
        }

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

        private Color _hColor = Colors.Red;
        public Color HColor
        {
            get => _hColor;
            set
            {
                _hColor = value;
                UpdatePens();
            }
        }

        private Color _vColor = Colors.Lime;
        public Color VColor
        {
            get => _vColor;
            set
            {
                _vColor = value;
                UpdatePens();
            }
        }

        private Color _cColor = Colors.White;
        public Color CColor
        {
            get => _cColor;
            set
            {
                _cColor = value;
                UpdatePens();
            }
        }

        private SolidColorBrush hBrush, vBrush, cBrush;
        private Pen hPen, vPen, cPen;

        public ODCrosshairElement()
        {
            UpdatePens();
        }

        private void UpdatePens()
        {
            hBrush = new SolidColorBrush(_hColor);
            vBrush = new SolidColorBrush(_vColor);
            cBrush = new SolidColorBrush(_cColor);

            hPen = new Pen(hBrush, _thickness);
            vPen = new Pen(vBrush, _thickness);
            cPen = new Pen(cBrush, _thickness);
        }

        public void SetColours(string hCol, string vCol, string cCol)
        {
            HColor = Avalonia.Media.Color.Parse(hCol);
            VColor = Avalonia.Media.Color.Parse(vCol);
            CColor = Avalonia.Media.Color.Parse(cCol);
        }

        public override void Draw(DrawingContext context, float scale, ODPoint vpWorldSize, ODPoint worldMousePosition)
        {
            if (!IsVisible)
                return;

            Center = worldMousePosition;

            float cSize = Size / scale; // Size still scales with zoom
                                        // Use constant thickness in screen pixels

            _thickness = Weight / scale;
            hPen.Thickness = _thickness;
            vPen.Thickness = _thickness;
            cPen.Thickness = _thickness;


            // Centre square
            context.DrawRectangle(null, cPen,
                new Rect(
                    new Point(Center.X - cSize / 2, Center.Y - cSize / 2),
                    new Size(cSize, cSize)
                ));

            var top = new Point(Center.X, Center.Y + vpWorldSize.Y);
            var bottom = new Point(Center.X, Center.Y - vpWorldSize.Y);
            var left = new Point(Center.X - vpWorldSize.X, Center.Y);
            var right = new Point(Center.X + vpWorldSize.X, Center.Y);

            context.DrawLine(vPen, top, new Point(Center.X, Center.Y + cSize / 2));
            context.DrawLine(vPen, new Point(Center.X, Center.Y - cSize / 2), bottom);
            context.DrawLine(hPen, left, new Point(Center.X - cSize / 2, Center.Y));
            context.DrawLine(hPen, new Point(Center.X + cSize / 2, Center.Y), right);
        }

    }
}

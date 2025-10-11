using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Input;
using Avalonia.Media;
using OpenDraft.Core.ODData;
using OpenDraft.Core.ODGeometry;
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Reflection.Metadata;

namespace OpenDraft
{
    public class ViewportCamera
    {
        public Point Position { get; set; }
        public float Scale { get; set; }

        public ViewportCamera()
        {
            Position = new Point(0,0);
            Scale = 1.0f;
        }

        public void MoveBy(float dX, float dY)
        {
            Position = new Point(Position.X + dX, Position.Y + dY);
        }
    }

    public partial class Viewport : UserControl
    {

        public static readonly StyledProperty<IEnumerable<ODElement>> ElementsProperty =
            AvaloniaProperty.Register<Viewport, IEnumerable<ODElement>>(nameof(Elements));

        public static readonly StyledProperty<ODLayerManager> LayerManagerProperty =
            AvaloniaProperty.Register<Viewport, ODLayerManager>(nameof(LayerManager));


        public IEnumerable<ODElement> Elements
        {
            get => GetValue(ElementsProperty);
            set => SetValue(ElementsProperty, value);
        }

        // Add LayerManager property
        public ODLayerManager LayerManager
        {
            get => GetValue(LayerManagerProperty);
            set => SetValue(LayerManagerProperty, value);
        }


        // Camera and interaction state
        ViewportCamera Camera = new ViewportCamera();
        bool isPanning = false;
        Point _lastPointerPosition = new Point(0, 0);
        Point _lastSize = new Point(0, 0);
        private bool _isInitialized = false; // Flag to run setup only once


        public Viewport()
        {
            InitializeComponent();
            
            // Set up event handlers
            this.Background = Brushes.Transparent;
            this.PointerPressed += OnPointerPressed;
            this.PointerWheelChanged += OnPointerWheelChanged;
            this.PointerReleased += OnPointerReleased;
            this.PointerMoved += OnPointerMoved;
        
            InvalidateVisual();
        }



        /* Events */

        private static void OnElementsChanged(AvaloniaObject d, AvaloniaPropertyChangedEventArgs e)
        {
            Debug.WriteLine("Elements property changed, invalidating visual.");
            // 'd' is the control itself. We check if it's a Viewport.
            if (d is Viewport viewport)
            {
                viewport.InvalidateVisual();
            }
        }

        protected override Size ArrangeOverride(Size finalSize)
        {
            bool needsRedraw = false;

            // --- ONE-TIME INITIALIZATION ---
            // Check for a valid size (> 0) and that we haven't initialized yet.
            if (!_isInitialized && finalSize.Width > 0 && finalSize.Height > 0)
            {
                _lastSize = new Point(finalSize.Width, finalSize.Height);
                Debug.WriteLine($"Initial size detected: {_lastSize}");

                // Centre the camera using the initial size.
                // The camera's position is in world coordinates, so we divide by scale.
                Camera.Position = new Point(
                    -(_lastSize.X / 2) / Camera.Scale,
                    -(_lastSize.Y / 2) / Camera.Scale
                );

                _isInitialized = true;
                needsRedraw = true;
            }
            // --- SUBSEQUENT RESIZE HANDLING ---
            // Only run this if we are already initialized and the size has changed.
            else if (_isInitialized && (finalSize.Width != _lastSize.X || finalSize.Height != _lastSize.Y))
            {
                double changeInWidth = finalSize.Width - _lastSize.X;
                double changeInHeight = finalSize.Height - _lastSize.Y;

                // Adjust camera position to keep the view centered on resize
                Camera.Position = new Point(
                    Camera.Position.X - (changeInWidth / 2 / Camera.Scale),
                    Camera.Position.Y - (changeInHeight / 2 / Camera.Scale)
                );

                _lastSize = new Point(finalSize.Width, finalSize.Height);
                needsRedraw = true;
            }

            if (needsRedraw)
            {
                InvalidateVisual(); // Trigger a redraw if size has changed
            }

            return base.ArrangeOverride(finalSize);
        }

        private void OnPointerPressed(object? sender, PointerPressedEventArgs e)
        {
            var props = e.GetCurrentPoint(this).Properties;
            if (props.IsLeftButtonPressed)
            {
                System.Diagnostics.Debug.WriteLine($"Left mouse button DOWN");
            }
            else if (props.IsMiddleButtonPressed)
            {
                var point = e.GetPosition(this);
                isPanning = true;
                System.Diagnostics.Debug.WriteLine($"Middle mouse button DOWN at {point}");
            }
            else if (props.IsRightButtonPressed)
            {
                System.Diagnostics.Debug.WriteLine($"Right mouse button DOWN");
            }
        }

        private void OnPointerReleased(object? sender, PointerReleasedEventArgs e)
        {
            var point = e.GetPosition(this);

            switch (e.InitialPressMouseButton) // which button triggered the press
            {
                case MouseButton.Left:
                    System.Diagnostics.Debug.WriteLine($"Left mouse button UP at {point}");
                    break;

                case MouseButton.Middle:
                    isPanning = false; // reset panning here
                    System.Diagnostics.Debug.WriteLine($"Middle mouse button UP at {point}");
                    break;

                case MouseButton.Right:
                    System.Diagnostics.Debug.WriteLine($"Right mouse button UP at {point}");
                    break;
            }
        }

        private void OnPointerWheelChanged(object? sender, PointerWheelEventArgs e)
        {
            var mousePos = e.GetPosition(this);

            // Exponential zoom
            float zoomFactor = 1.1f;
            float zoomDirection = (float)e.Delta.Y > 0 ? 1f : -1f;
            float oldScale = Camera.Scale;
            float newScale = oldScale * (float)Math.Pow(zoomFactor, zoomDirection);

            // Calculate world position under cursor before zoom
            double worldX = (mousePos.X / oldScale) - Camera.Position.X;
            double worldY = ((Bounds.Height - mousePos.Y) / oldScale) - Camera.Position.Y; // Inverted Y

            // Calculate world position under cursor after zoom
            double newWorldX = (mousePos.X / newScale) - Camera.Position.X;
            double newWorldY = ((Bounds.Height - mousePos.Y) / newScale) - Camera.Position.Y; // Inverted Y

            // Adjust camera position to keep the world point under cursor fixed
            Camera.Position = new Point(
                Camera.Position.X + (worldX - newWorldX),
                Camera.Position.Y + (worldY - newWorldY)
            );

            Camera.Scale = newScale;
            InvalidateVisual();
        }


        private void OnPointerMoved(object? sender, PointerEventArgs e)
        {
            Point current = e.GetPosition(this);
            Point delta = current - _lastPointerPosition; // delta as a Vector

            if (isPanning)
            {
                Camera.MoveBy((float)-delta.X / Camera.Scale, (float)delta.Y / Camera.Scale);
                InvalidateVisual();

                Debug.WriteLine($"Camera position {Camera.Position.ToString()}");
            }

            _lastPointerPosition = current;

        }



        /* Rendering */

        public override void Render(DrawingContext context)
        {
            base.Render(context);
            var bounds = new Rect(0, 0, Bounds.Width, Bounds.Height);
            
            using (context.PushClip(bounds))
                drawScene(context);
            
        }



        private void drawScene(DrawingContext context)
        {
            // Create the camera transform matrix with inverted Y
            var matrix = new Matrix(
                Camera.Scale, 0,
                0, -Camera.Scale, // Negative for Y inversion
                -Camera.Position.X * Camera.Scale,
                (Camera.Position.Y * Camera.Scale) + Bounds.Height // Adjust for Y inversion
            );

            // Push the matrix directly
            using (context.PushTransform(matrix))
            {
                foreach (var element in Elements)
                {
                    ODLayer? layer = LayerManager.GetLayerByID(element.LayerId);

                    if (layer != null)
                        element.Draw(context, layer);
                }
            }
        }







    }
}

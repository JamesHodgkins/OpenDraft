using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.ObjectModel;
using System.Diagnostics;


namespace OpenDraft
{
    public class ViewportCamera
    {
        public Point Position { get; set; }
        public float Scale { get; set; }

        public ViewportCamera(Point? position = null, float scale = 1.0f)
        {
            Position = position ?? new Point(0, 0);
            Scale = scale;
        }

        public void MoveBy(Vector delta) => Position += new Point(delta.X, delta.Y);
    }

    public partial class Viewport : UserControl
    {
        public static readonly StyledProperty<ObservableCollection<ODElement>> ElementsProperty =
            AvaloniaProperty.Register<Viewport, ObservableCollection<ODElement>>(nameof(Elements));

        public static readonly StyledProperty<ODLayerManager> LayerManagerProperty =
            AvaloniaProperty.Register<Viewport, ODLayerManager>(nameof(LayerManager));

        public static readonly StyledProperty<ODEditor> EditorProperty =
            AvaloniaProperty.Register<Viewport, ODEditor>(nameof(Editor));

        public static readonly StyledProperty<IODEditorInputService> InputServiceProperty =
        AvaloniaProperty.Register<Viewport, IODEditorInputService>(nameof(InputService));

        public static readonly StyledProperty<ObservableCollection<ODDynamicElement>> DynamicElementsProperty =
        AvaloniaProperty.Register<Viewport, ObservableCollection<ODDynamicElement>>(nameof(DynamicElements));

        public ObservableCollection<ODElement> Elements
        {
            get => GetValue(ElementsProperty);
            set => SetValue(ElementsProperty, value);
        }

        public ODLayerManager LayerManager
        {
            get => GetValue(LayerManagerProperty);
            set => SetValue(LayerManagerProperty, value);
        }

        public ODEditor Editor
        {
            get => GetValue(EditorProperty);
            set => SetValue(EditorProperty, value);
        }

        public IODEditorInputService InputService
        {
            get => GetValue(InputServiceProperty);
            set => SetValue(InputServiceProperty, value);
        }

        public ObservableCollection<ODDynamicElement> DynamicElements
        {
            get => GetValue(DynamicElementsProperty);
            set => SetValue(DynamicElementsProperty, value);
        }

        private readonly ViewportCamera Camera = new();
        private bool isPanning = false;
        private Point _lastPointerDragPosition;
        private Point _mousePosition;
        private Size _lastSize;
        private bool _isInitialized = false;

        public Viewport()
        {
            InitializeComponent();
            Background = Brushes.Transparent;

            Elements ??= new ObservableCollection<ODElement>();
            DynamicElements ??= new ObservableCollection<ODDynamicElement>();

            SubscribeToElements();
            SubscribeToDynamicElements();

            // ADD DEBUG
            Debug.WriteLine($"Viewport constructor - Elements: {Elements?.Count}, DynamicElements: {DynamicElements?.Count}");

            PointerPressed += OnPointerPressed;
            PointerReleased += OnPointerReleased;
            PointerMoved += OnPointerMoved;
            PointerWheelChanged += OnPointerWheelChanged;

            InvalidateVisual();
        }

        private void SubscribeToElements()
        {
            if (Elements != null)
            {
                Elements.CollectionChanged -= OnElementsChanged;
                Elements.CollectionChanged += OnElementsChanged;
            }
        }

        private void SubscribeToDynamicElements()
        {
            if (DynamicElements != null)
            {
                DynamicElements.CollectionChanged -= OnDynamicElementsChanged;
                DynamicElements.CollectionChanged += OnDynamicElementsChanged;
            }
        }

        private void OnElementsChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            InvalidateVisual();
        }

        private void OnDynamicElementsChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            InvalidateVisual();
        }

        protected override Size ArrangeOverride(Size finalSize)
        {
            if (!_isInitialized)
            {
                _isInitialized = true;
                Camera.Position = new Point(-finalSize.Width / 2 / Camera.Scale,
                                            -finalSize.Height / 2 / Camera.Scale);
            }
            else
            {
                // Keep view centered on resize
                Camera.Position -= new Point(
                    (finalSize.Width - _lastSize.Width) / 2 / Camera.Scale,
                    (finalSize.Height - _lastSize.Height) / 2 / Camera.Scale
                );
            }

            _lastSize = finalSize;

            // Force redraw
            InvalidateVisual();

            return base.ArrangeOverride(finalSize);
        }


        protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
        {
            base.OnPropertyChanged(change);

            if (change.Property == ElementsProperty)
            {
                Debug.WriteLine($"Elements property changed: {Elements?.Count}");
                SubscribeToElements();
            }
            else if (change.Property == DynamicElementsProperty)
            {
                Debug.WriteLine($"DynamicElements property changed: {DynamicElements?.Count}");
                SubscribeToDynamicElements();
            }
            else if (change.Property == EditorProperty)
            {
                Debug.WriteLine($"Editor property changed: {Editor != null}");
                // Check if we can access Editor.DynamicElements
                if (Editor != null)
                {
                    Debug.WriteLine($"Editor.DynamicElements count: {Editor.DynamicElements?.Count}");
                }
            }
        }


        private void OnPointerPressed(object? sender, PointerPressedEventArgs e)
        {
            if (e.GetCurrentPoint(this).Properties.IsMiddleButtonPressed)
            {
                _lastPointerDragPosition = e.GetPosition(this);
                isPanning = true;
            }
        }

        private void OnPointerReleased(object? sender, PointerReleasedEventArgs e)
        {
            if (e.InitialPressMouseButton == MouseButton.Left)
            {
                Point worldPos = GetWorldMousePosition();
                var odPoint = new ODPoint(worldPos.X, worldPos.Y);
                
                // Use input service instead of direct editor call
                InputService?.RaisePointProvided(odPoint);
                e.Handled = true;
            }

            if (e.InitialPressMouseButton == MouseButton.Middle)
                isPanning = false;
        }

        private void OnPointerMoved(object? sender, PointerEventArgs e)
        {
            _mousePosition = e.GetPosition(this);
            Point world = GetWorldMousePosition();
            

            if (!isPanning) return;

            Point current = e.GetPosition(this);
            Camera.MoveBy(new Vector(-(current.X - _lastPointerDragPosition.X) / Camera.Scale,
                                     (current.Y - _lastPointerDragPosition.Y) / Camera.Scale));
            _lastPointerDragPosition = current;

            InvalidateVisual();
        }

        private void OnPointerWheelChanged(object? sender, PointerWheelEventArgs e)
        {
            Point mousePos = e.GetPosition(this);

            float zoomFactor = 1.1f;
            float oldScale = Camera.Scale;
            float newScale = oldScale * (float)Math.Pow(zoomFactor, e.Delta.Y > 0 ? 1 : -1);

            Point ScreenToWorld(Point screen, float scale) =>
                new(screen.X / scale - Camera.Position.X, (Bounds.Height - screen.Y) / scale - Camera.Position.Y);

            var oldWorld = ScreenToWorld(mousePos, oldScale);
            var newWorld = ScreenToWorld(mousePos, newScale);

            Camera.Position += oldWorld - newWorld;
            Camera.Scale = newScale;

            InvalidateVisual();
        }

        public override void Render(DrawingContext context)
        {
            base.Render(context);

            var bounds = new Rect(0, 0, Bounds.Width, Bounds.Height);
            using (context.PushClip(bounds))
            {
                DrawScene(context);
            }
        }

        public Point GetScreenMousePosition()
        {
             return _mousePosition;
        }

        public Point GetWorldMousePosition()
        {
            return ScreenToWorld(_mousePosition);
        }

        private Point ScreenToWorld(Point screenPoint)
        {
            return new Point(
                screenPoint.X / Camera.Scale + Camera.Position.X,
                (Bounds.Height - screenPoint.Y) / Camera.Scale + Camera.Position.Y
            );
        }

        private void DrawScene(DrawingContext context)
        {
            if ((Elements == null || Elements.Count == 0) &&
                    (DynamicElements == null || DynamicElements.Count == 0))
                return;

            var matrix = new Matrix(
                Camera.Scale, 0,
                0, -Camera.Scale,
                -Camera.Position.X * Camera.Scale,
                Camera.Position.Y * Camera.Scale + Bounds.Height
            );

            // Draw static elements first
            if (Elements != null)
            {
                using (context.PushTransform(matrix))
                {
                    foreach (var element in Elements)
                    {
                        var layer = LayerManager?.GetLayerByID(element.LayerId);

                        if (layer != null)
                            element.Draw(context, layer);
                    }
                    
                    // Draw dynamic elements on top
                    if (DynamicElements != null)
                    {
                        foreach (var element in DynamicElements)
                        {
                            element.Draw(context);
                        }
                    }
                }

            }
        }
    }
}

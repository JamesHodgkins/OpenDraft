using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.ObjectModel;

namespace OpenDraft
{
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

            SubscribeToCollection(Elements, OnElementsChanged);
            SubscribeToCollection(DynamicElements, OnDynamicElementsChanged);

            PointerPressed += OnPointerPressed;
            PointerReleased += OnPointerReleased;
            PointerMoved += OnPointerMoved;
            PointerWheelChanged += OnPointerWheelChanged;

            SetupStaticCanvas();
            SetupDynamicCanvas();
        }

        private void SubscribeToCollection<T>(ObservableCollection<T> collection, System.Collections.Specialized.NotifyCollectionChangedEventHandler handler)
        {
            if (collection == null) return;
            collection.CollectionChanged -= handler;
            collection.CollectionChanged += handler;
        }

        private void OnElementsChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            StaticCanvas?.InvalidateVisual();
        }

        private void OnDynamicElementsChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            DynamicCanvas?.InvalidateVisual();
        }

        private void SetupStaticCanvas()
        {
            StaticCanvas.OnRender = (context, bounds) =>
            {
                if (Elements == null) return;
                var matrix = Camera.GetMatrix(bounds.Height);

                using (context.PushTransform(matrix))
                {
                    foreach (var element in Elements)
                    {
                        var layer = LayerManager?.GetLayerByID(element.LayerId);
                        if (layer != null)
                            element.Draw(context, layer);
                    }
                }
            };
            StaticCanvas.InvalidateVisual();
        }

        private void SetupDynamicCanvas()
        {
            DynamicCanvas.OnRender = (context, bounds) =>
            {
                var matrix = Camera.GetMatrix(bounds.Height);
                using (context.PushTransform(matrix))
                {
                    if (DynamicElements != null)
                        foreach (var element in DynamicElements) element.Draw(context);

                    if (Editor?.DynamicElements != null)
                        foreach (var element in Editor.DynamicElements) element.Draw(context);
                }
            };

            var timer = new DispatcherTimer { Interval = TimeSpan.FromMilliseconds(16) };
            timer.Tick += (_, _) => DynamicCanvas?.InvalidateVisual();
            timer.Start();
        }

        protected override Size ArrangeOverride(Size finalSize)
        {
            if (!_isInitialized)
            {
                _isInitialized = true;
                Camera.Position = new Point(-finalSize.Width / 2 / Camera.Scale, -finalSize.Height / 2 / Camera.Scale);
            }
            else
            {
                Camera.Position -= new Vector(
                    (finalSize.Width - _lastSize.Width) / 2 / Camera.Scale,
                    (finalSize.Height - _lastSize.Height) / 2 / Camera.Scale);
            }

            _lastSize = finalSize;
            StaticCanvas?.InvalidateVisual();
            return base.ArrangeOverride(finalSize);
        }

        protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
        {
            base.OnPropertyChanged(change);
            if (!_isInitialized) return;

            if (change.Property == ElementsProperty)
                SubscribeToCollection(Elements, OnElementsChanged);
            else if (change.Property == DynamicElementsProperty)
                SubscribeToCollection(DynamicElements, OnDynamicElementsChanged);

            StaticCanvas?.InvalidateVisual();
            DynamicCanvas?.InvalidateVisual();
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
                var odPoint = new ODPoint(GetWorldMousePosition().X, GetWorldMousePosition().Y);
                InputService?.RaisePointProvided(odPoint);
                e.Handled = true;
            }

            if (e.InitialPressMouseButton == MouseButton.Middle)
                isPanning = false;
        }

        private void OnPointerMoved(object? sender, PointerEventArgs e)
        {
            _mousePosition = e.GetPosition(this);
            Editor?.UpdateCrosshairPosition(new ODPoint(GetWorldMousePosition().X, GetWorldMousePosition().Y));

            DynamicCanvas?.InvalidateVisual();

            if (!isPanning) return;

            var current = e.GetPosition(this);
            Camera.MoveBy(new Vector(-(current.X - _lastPointerDragPosition.X) / Camera.Scale,
                                     (current.Y - _lastPointerDragPosition.Y) / Camera.Scale));
            _lastPointerDragPosition = current;

            StaticCanvas?.InvalidateVisual();
        }

        private void OnPointerWheelChanged(object? sender, PointerWheelEventArgs e)
        {
            var mousePos = e.GetPosition(this);
            float zoomFactor = 1.1f;
            float oldScale = Camera.Scale;
            float newScale = oldScale * (float)Math.Pow(zoomFactor, e.Delta.Y > 0 ? 1 : -1);

            Point ScreenToWorld(Point screen, float scale) =>
                new(screen.X / scale - Camera.Position.X, (Bounds.Height - screen.Y) / scale - Camera.Position.Y);

            var oldWorld = ScreenToWorld(mousePos, oldScale);
            var newWorld = ScreenToWorld(mousePos, newScale);

            Camera.Position += oldWorld - newWorld; // <--- fix here
            Camera.Scale = newScale;

            StaticCanvas?.InvalidateVisual();
            DynamicCanvas?.InvalidateVisual();
        }


        public Point GetScreenMousePosition() => _mousePosition;

        public Point GetWorldMousePosition() =>
            new Point(_mousePosition.X / Camera.Scale + Camera.Position.X,
                      (Bounds.Height - _mousePosition.Y) / Camera.Scale + Camera.Position.Y);
    }
}

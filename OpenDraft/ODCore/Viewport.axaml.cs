using Avalonia;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Media;
using Avalonia.Threading;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using OpenDraft.ODCore.ODSystem;
using OpenDraft.ViewModels;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;

namespace OpenDraft
{
    public partial class Viewport : UserControl
    {
        public static readonly StyledProperty<ObservableCollection<ODElement>> ElementsProperty =
            AvaloniaProperty.Register<Viewport, ObservableCollection<ODElement>>(nameof(Elements));

        public static readonly StyledProperty<ObservableCollection<ODDynamicElement>> DynamicElementsProperty =
            AvaloniaProperty.Register<Viewport, ObservableCollection<ODDynamicElement>>(nameof(DynamicElements));
        
        public static readonly StyledProperty<ODDataService> DataServiceProperty =
            AvaloniaProperty.Register<Viewport, ODDataService>(nameof(DataService));

        public static readonly StyledProperty<ODEditorHandler> EditorHandlerProperty =
            AvaloniaProperty.Register<Viewport, ODEditorHandler>(nameof(EditorHandler));

        public static readonly StyledProperty<IODEditorInputService> InputServiceProperty =
            AvaloniaProperty.Register<Viewport, IODEditorInputService>(nameof(InputService));


        public ObservableCollection<ODElement> Elements
        {
            get => GetValue(ElementsProperty);
            set => SetValue(ElementsProperty, value);
        }

        public ObservableCollection<ODDynamicElement> DynamicElements
        {
            get => GetValue(DynamicElementsProperty);
            set => SetValue(DynamicElementsProperty, value);
        }

        public ODDataService DataService
        {
            get => GetValue(DataServiceProperty);
            set => SetValue(DataServiceProperty, value);
        }

        public ODEditorHandler EditorHandler
        {
            get => GetValue(EditorHandlerProperty);
            set => SetValue(EditorHandlerProperty, value);
        }

        public IODEditorInputService InputService
        {
            get => GetValue(InputServiceProperty);
            set => SetValue(InputServiceProperty, value);
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

            SetupStaticCanvas();
            SetupDynamicCanvas();

            PointerPressed += OnPointerPressed;
            PointerReleased += OnPointerReleased;
            PointerMoved += OnPointerMoved;
            PointerWheelChanged += OnPointerWheelChanged;
            PointerEntered += OnPointerEntered;
            PointerExited += OnPointerExited;
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
                        var layer = DataService.GetLayerByID(element.LayerId);
                        if (layer != null)
                            element.Draw(context, DataService);
                    }
                }
            };
            StaticCanvas.InvalidateVisual();
        }

        private void SetupDynamicCanvas()
        {
            DynamicCanvas.OnRender = (context, bounds) =>
            {
                if (DynamicElements == null) return;

                ODVec2 vpWorldSize = new ODVec2(bounds.Width / Camera.Scale, bounds.Height / Camera.Scale);
                ODVec2 worldMousePoint = new ODVec2(GetWorldMousePosition().X, GetWorldMousePosition().Y);

                var matrix = Camera.GetMatrix(bounds.Height);
                using (context.PushTransform(matrix))
                {
                    if (DynamicElements != null)
                        foreach (ODDynamicElement element in DynamicElements)
                        {
                            ODLayer? layer = DataService.GetLayerByID(element.LayerId);
                            if (layer != null) element.Draw(context, DataService, Camera.Scale, vpWorldSize, worldMousePoint);
                        }

                    ODElement? highlightedElement = EditorHandler.GetHighlighted();
                    string highlightColourHex = ODSystem.GetRegistryValueAsString("style/highlight_colour") ?? "#FFFFFF";
                    int hIntensity = ODSystem.GetRegistryValueAsInt("style/highlight_intensity") ?? 0;
                    ODColour hColour = new ODColour(highlightColourHex);

                    if (highlightedElement != null)
                        highlightedElement.DrawHighlight(context, DataService, hColour, hIntensity);
                    
                    ODSelectionSet? selectedElements = EditorHandler.GetActiveSelection();
                    if (selectedElements == null)
                        return;

                    
                    int num = selectedElements.Count;
                    
                    foreach (ODElement element in selectedElements.SelectedElements)
                    {
                        element.DrawHighlight(context, DataService, hColour, 200);
                    }
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

            // Elements subscription
            if (change.Property == ElementsProperty)
            {
                if (change.OldValue is ObservableCollection<ODElement> oldCollection)
                    oldCollection.CollectionChanged -= OnElementsChanged;

                if (change.NewValue is ObservableCollection<ODElement> newCollection)
                    newCollection.CollectionChanged += OnElementsChanged;

                StaticCanvas?.InvalidateVisual();
            }

            // DynamicElements subscription
            else if (change.Property == DynamicElementsProperty)
            {
                if (change.OldValue is ObservableCollection<ODDynamicElement> oldCollection)
                    oldCollection.CollectionChanged -= OnDynamicElementsChanged;

                if (change.NewValue is ObservableCollection<ODDynamicElement> newCollection)
                    newCollection.CollectionChanged += OnDynamicElementsChanged;

                DynamicCanvas?.InvalidateVisual();
            }
        }

        private void OnElementsChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        => StaticCanvas?.InvalidateVisual();

        private void OnDynamicElementsChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
            => DynamicCanvas?.InvalidateVisual();


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
                // Update Editor
                bool shiftDown = e.KeyModifiers.HasFlag(KeyModifiers.Shift);
                EditorHandler.LeftMouseClicked(shiftDown);

                // Trigger input
                var ODVec2 = new ODVec2(GetWorldMousePosition().X, GetWorldMousePosition().Y);
                InputService?.RaisePointProvided(ODVec2);
                e.Handled = true;
            }

            if (e.InitialPressMouseButton == MouseButton.Middle)
                isPanning = false;
        }

        private void OnPointerMoved(object? sender, PointerEventArgs e)
        {
            _mousePosition = e.GetPosition(this);
            DynamicCanvas?.InvalidateVisual();
            EditorHandler.UpdateMousePosition(GetWorldMousePosition(), Camera.Scale);
            

            if (!isPanning) return;

            var current = e.GetPosition(this);
            Camera.MoveBy(new Vector(-(current.X - _lastPointerDragPosition.X) / Camera.Scale,
                                     (current.Y - _lastPointerDragPosition.Y) / Camera.Scale));
            _lastPointerDragPosition = current;

            StaticCanvas?.InvalidateVisual();

        }

        private void OnPointerEntered(object? sender, PointerEventArgs e)
        {
            InputService?.RaiseViewportEntered();
        }

        private void OnPointerExited(object? sender, PointerEventArgs e)
        {
            InputService?.RaiseViewportExited();
        }

        private void OnPointerWheelChanged(object? sender, PointerWheelEventArgs e)
        {
            var mousePos = e.GetPosition(this);
            double zoomFactor = 1.1f;
            double oldScale = Camera.Scale;
            double newScale = oldScale * Math.Pow(zoomFactor, e.Delta.Y > 0 ? 1 : -1);

            Point ScreenToWorld(Point screen, double scale) =>
                new(screen.X / scale - Camera.Position.X, (Bounds.Height - screen.Y) / scale - Camera.Position.Y);

            var oldWorld = ScreenToWorld(mousePos, oldScale);
            var newWorld = ScreenToWorld(mousePos, newScale);

            Camera.Position += oldWorld - newWorld; // <--- fix here
            Camera.Scale = newScale;

            StaticCanvas?.InvalidateVisual();
            DynamicCanvas?.InvalidateVisual();
        }


        public Point GetScreenMousePosition() => _mousePosition;

        public ODVec2 GetWorldMousePosition()
        {
            double x = (_mousePosition.X / Camera.Scale + Camera.Position.X);
            double y = ((Bounds.Height - _mousePosition.Y) / Camera.Scale) + Camera.Position.Y;
            return new ODVec2(x,y);
        }
    }
}

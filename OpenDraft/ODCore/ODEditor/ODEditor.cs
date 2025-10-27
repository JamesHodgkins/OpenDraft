using Avalonia.Input;
using Avalonia.Threading;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODEditor.ODDynamics;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Diagnostics;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor
{
    public class ODEditor : IDisposable
    {
        private readonly ODDataManager _dataManager;
        private readonly ODCommandRegistry _commandRegistry;
        private readonly IODEditorInputService _inputService;
        private readonly ODSelectionManager _selectionManager;
        private ODElement? _highlightedElement = null;

        private ODVec2 mousePosition = new ODVec2(0,0);


        public ObservableCollection<ODDynamicElement> DynamicElements { get; } = new();

        private ODEditorContext? _currentContext;
        private IODEditorCommand? _currentCommand;

        public event EventHandler<MessageEventArgs>? ShowMessageRequested;
        public event EventHandler<MessageEventArgs>? StatusMessageChanged;

        ODCrosshairElement ch = new ODCrosshairElement();


        public ODEditor(ODDataManager dataManager, IODEditorInputService inputService)
        {
            _dataManager = dataManager;
            _inputService = inputService;
            _commandRegistry = new ODCommandRegistry();
            _commandRegistry.RegisterAssembly(typeof(ODEditor).Assembly);
            _selectionManager = new ODSelectionManager();

            // Subscribe to input events
            _inputService.KeyPressed += OnKeyPressed;
            _inputService.CancelRequested += OnCancelRequested;

            // Subscribe to viewport focus events
            _inputService.ViewportEntered += OnViewportEntered;
            _inputService.ViewportExited += OnViewportExited;

            DynamicElements.Add(ch);
        }

        // Add event handlers
        private void OnViewportEntered()
        {
            ShowCrosshair();
        }

        private void OnViewportExited()
        {
            HideCrosshair();
        }

        public void UpdateMousePosition(ODVec2 point, double viewportScale)
        {
            mousePosition = point;

            _highlightedElement = null;
            double tolerance = (ODSystem.ODSystem.GetRegistryValueAsInt("system/select_tolerance") ?? 10.0) / viewportScale;
            
            foreach (ODElement element in _dataManager.Elements)
            {
                if (element.HitTest(point, tolerance))
                    _highlightedElement = element;       
            }
        }

        public void LeftMouseClicked(bool shiftDown)
        {
            if (_highlightedElement != null)
            {
                ODSelectionSet? sSet = _selectionManager.GetActiveSelectionSet();

                if (!shiftDown)
                {
                    sSet.AddElement(_highlightedElement);
                }
                else
                {
                    sSet.RemoveElement(_highlightedElement);
                }
                _selectionManager.UpdateSelectionSet(sSet);
            }
        }

        public ODVec2 GetMousePosition()
        {
            return mousePosition;
        }

        public ODElement? GetHighlighted()
        {
            return _highlightedElement;
        }

        public ODSelectionSet? GetActiveSelection()
        {
            return _selectionManager.GetActiveSelectionSet();
        }

        /* DYNAMIC ELEMENTS MANAGEMENT */

        public void restoreDefaultDynamicElements()
        {
            DynamicElements.Clear();
            // Future: Add default dynamic elements if any <<<< TODO IMPLEMENT <<<<
            DynamicElements.Add(ch);
        }

        public void ClearDynamicElements()
        {
            DynamicElements.Clear();
            DynamicElements.Add(ch);
        }

        // ADD THIS: Method to add dynamic elements
        public void AddDynamicElement(ODDynamicElement element)
        {
            Debug.WriteLine("AddDynamicElement: " + element.GetType().Name);
            DynamicElements.Add(element);
        }

        // ADD THIS: Method to remove specific dynamic element
        public void RemoveDynamicElement(ODDynamicElement element)
        {
            DynamicElements.Remove(element);
        }

        public void AddStaticElement(ODElement element)
        {
            _dataManager.AddElement(element);
        }


        /* COMMAND EXECUTION AND INPUT HANDLING  */

        private void OnKeyPressed(Key key)
        {
            // Handle specific keys we care about
            switch (key)
            {
                case Key.Escape:
                    CancelCurrentCommand();
                    break;
                    // Future: handle Enter, Space, etc.
            }
        }

        private void OnCancelRequested()
        {
            CancelCurrentCommand();
        }

        
        public void CancelCurrentCommand()
        {
            restoreDefaultDynamicElements();

            _currentContext?.Cancel();
            _currentContext = null;
            _currentCommand = null;
            SetStatus("Command cancelled");
        }

        public void ExecuteCommand(string commandName)
        {
            Debug.WriteLine($"ExecuteCommand: {commandName}");

            CancelCurrentCommand();

            var command = _commandRegistry.CreateCommand(commandName);
            if (command == null)
            {
                SetStatus($"Unknown command: {commandName}");
                return;
            }

            _currentCommand = command;
            _currentContext = new ODEditorContext(this, _dataManager, _inputService);

            async Task RunCommandAsync()
            {
                try
                {
                    await command.ExecuteAsync(_currentContext);
                }
                catch (Exception ex)
                {
                    SetStatus($"Error in {commandName}: {ex.Message}");
                    Debug.WriteLine($"Command error: {ex.Message}");
                }
                finally
                {
                    if (_currentCommand == command)
                    {
                        CancelCurrentCommand();
                    }
                }
            }

            _ = RunCommandAsync();
        }

        
        public void SetStatus(string message)
        {
            Debug.WriteLine("SetStatus: " + message);
            Dispatcher.UIThread.Post(() =>
            {
                StatusMessageChanged?.Invoke(this, new MessageEventArgs(message));
            });
        }

        public void ShowMessage(string message)
        {
            Debug.WriteLine("ShowMessage: " + message);
            Dispatcher.UIThread.Post(() =>
            {
                ShowMessageRequested?.Invoke(this, new MessageEventArgs(message));
            });
        }

        public void Dispose()
        {
            _inputService.KeyPressed -= OnKeyPressed;
            _inputService.CancelRequested -= OnCancelRequested;
            CancelCurrentCommand();
        }

        public void HideCrosshair()
        {
            ch.IsVisible = false;
        }

        public void ShowCrosshair()
        {
            ch.IsVisible = true;
        }
    }

    
    public class MessageEventArgs : EventArgs
    {
        public string Message { get; }
        public MessageEventArgs(string message) => Message = message;
    }

    public class BoolEventArgs : EventArgs
    {
        public bool Value { get; }
        public BoolEventArgs(bool value) => Value = value;
    }
}
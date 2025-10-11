using Avalonia.Input;
using Avalonia.Threading;
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Diagnostics;
using System.Threading;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor
{
    public class ODEditor : IDisposable
    {
        private readonly ODDataManager _dataManager;
        private readonly ODCommandRegistry _commandRegistry;
        private readonly IEditorInputService _inputService;
        private ODEditorContext? _currentContext;
        private IODEditorCommand? _currentCommand;

        public event EventHandler<MessageEventArgs>? ShowMessageRequested;
        public event EventHandler<MessageEventArgs>? StatusMessageChanged;

        public ODEditor(ODDataManager dataManager, IEditorInputService inputService)
        {
            _dataManager = dataManager;
            _inputService = inputService;
            _commandRegistry = new ODCommandRegistry();
            _commandRegistry.RegisterAssembly(typeof(ODEditor).Assembly);

            // Subscribe to input events
            _inputService.KeyPressed += OnKeyPressed;
            _inputService.CancelRequested += OnCancelRequested;
        }

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

        // ADD THIS MISSING METHOD
        public void CancelCurrentCommand()
        {
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

        // ADD THESE MISSING METHODS
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
    }

    // MOVE THIS CLASS INSIDE ODEditor.cs OR CREATE SEPARATE FILE
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
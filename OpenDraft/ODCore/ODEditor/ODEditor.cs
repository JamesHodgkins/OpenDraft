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
        private ODEditorContext? _currentContext;
        private IODEditorCommand? _currentCommand;

        public event EventHandler<MessageEventArgs>? ShowMessageRequested;
        public event EventHandler<BoolEventArgs>? InputModeChanged;
        public event EventHandler<MessageEventArgs>? StatusMessageChanged;

        public ODEditor(ODDataManager dataManager)
        {
            _dataManager = dataManager;
            _commandRegistry = new ODCommandRegistry();
            _commandRegistry.RegisterAssembly(typeof(ODEditor).Assembly);
        }

        // THIS METHOD WAS ACCIDENTALLY REMOVED - RESTORE IT
        public void ExecuteCommand(string commandName)
        {
            Debug.WriteLine($"ExecuteCommand: {commandName}");

            // Cancel any running command and clear context
            CancelCurrentCommand();

            var command = _commandRegistry.CreateCommand(commandName);
            if (command == null)
            {
                SetStatus($"Unknown command: {commandName}");
                return;
            }

            _currentCommand = command;
            _currentContext = new ODEditorContext(this, _dataManager);

            // Use proper async execution
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

        public void HandlePointInput(ODPoint point)
        {
            _currentContext?.ProvidePoint(point);
        }

        public void HandleNumberInput(double number)
        {
            _currentContext?.ProvideNumber(number);
        }

        public void HandleTextInput(string text)
        {
            _currentContext?.ProvideText(text);
        }

        public void CancelCurrentCommand()
        {
            _currentContext?.Cancel();
            _currentContext = null;
            _currentCommand = null;
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
            CancelCurrentCommand();
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
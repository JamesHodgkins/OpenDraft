using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODEditor.ODCommands;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Threading;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor
{
    public class ODEditorContext : IODEditorGateway
    {
        private readonly ODEditor _editor;
        private readonly ODDataManager _dataManager;
        private readonly CancellationTokenSource _cancellationTokenSource;

        private TaskCompletionSource<ODPoint>? _pointWaiter;
        private TaskCompletionSource<double>? _numberWaiter;
        private TaskCompletionSource<string>? _textWaiter;
        private TaskCompletionSource<string>? _choiceWaiter;

        public ODDataManager DataManager => _dataManager;
        public CancellationToken CancellationToken => _cancellationTokenSource.Token;

        public ODEditorContext(ODEditor editor, ODDataManager dataManager)
        {
            _editor = editor;
            _dataManager = dataManager;
            _cancellationTokenSource = new CancellationTokenSource();
        }

        public Task<ODPoint> GetPointAsync(string prompt)
        {
            _editor.SetStatus(prompt);
            ClearAllWaiters();
            _pointWaiter = new TaskCompletionSource<ODPoint>();
            return _pointWaiter.Task;
        }

        public Task<double> GetNumberAsync(string prompt)
        {
            _editor.SetStatus(prompt);
            ClearAllWaiters();
            _numberWaiter = new TaskCompletionSource<double>();
            return _numberWaiter.Task;
        }

        public Task<string> GetTextAsync(string prompt)
        {
            _editor.SetStatus(prompt);
            ClearAllWaiters();
            _textWaiter = new TaskCompletionSource<string>();
            return _textWaiter.Task;
        }

        public Task<string> GetChoiceAsync(string prompt, params string[] options)
        {
            _editor.SetStatus(prompt);
            ClearAllWaiters();
            _choiceWaiter = new TaskCompletionSource<string>();
            return _choiceWaiter.Task;
        }

        public void ProvidePoint(ODPoint point)
        {
            if (point == null)
                throw new ArgumentNullException(nameof(point));

            if (_pointWaiter != null && !_pointWaiter.Task.IsCompleted)
            {
                _pointWaiter.TrySetResult(point);
            }
        }

        public void ProvideNumber(double number)
        {
            if (_numberWaiter != null && !_numberWaiter.Task.IsCompleted)
            {
                _numberWaiter.TrySetResult(number);
            }
        }

        public void ProvideText(string text)
        {
            if (_textWaiter != null && !_textWaiter.Task.IsCompleted)
            {
                _textWaiter.TrySetResult(text);
            }
        }

        public void ProvideChoice(string choice)
        {
            if (_choiceWaiter != null && !_choiceWaiter.Task.IsCompleted)
            {
                _choiceWaiter.TrySetResult(choice);
            }
        }

        public void Cancel()
        {
            _cancellationTokenSource.Cancel();
            _pointWaiter?.TrySetCanceled();
            _numberWaiter?.TrySetCanceled();
            _textWaiter?.TrySetCanceled();
            _choiceWaiter?.TrySetCanceled();
            ClearAllWaiters();
        }

        private void ClearAllWaiters()
        {
            _pointWaiter = null;
            _numberWaiter = null;
            _textWaiter = null;
            _choiceWaiter = null;
        }

        public void SetStatus(string message)
        {
            _editor.SetStatus(message);
        }

        public void ShowMessage(string message)
        {
            _editor.ShowMessage(message);
        }
    }
}
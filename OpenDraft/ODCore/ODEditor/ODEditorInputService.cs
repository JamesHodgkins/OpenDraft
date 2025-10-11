// EditorInputService.cs
using Avalonia.Input;
using OpenDraft.ODCore.ODGeometry;
using System;

namespace OpenDraft.ODCore.ODEditor
{
    public class ODEditorInputService : IODEditorInputService
    {
        public event Action<Key>? KeyPressed;
        public event Action<ODPoint>? PointProvided;
        public event Action<string>? TextInput;
        public event Action<double>? NumberInput;
        public event Action? CancelRequested;

        public void RaiseKeyPressed(Key key)
        {
            KeyPressed?.Invoke(key);
        }

        public void RaisePointProvided(ODPoint point)
        {
            PointProvided?.Invoke(point);
        }

        public void RaiseTextInput(string text)
        {
            TextInput?.Invoke(text);
        }

        public void RaiseNumberInput(double number)
        {
            NumberInput?.Invoke(number);
        }

        public void RaiseCancelRequested()
        {
            CancelRequested?.Invoke();
        }

        // These can be expanded later for more sophisticated input handling
        public void RequestPointInput() { /* Future: change cursor, show hints */ }
        public void RequestTextInput() { /* Future: focus text box */ }
        public void CancelInputRequest() { /* Future: reset cursor */ }
    }
}
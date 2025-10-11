// IEditorInputService.cs
using Avalonia.Input;
using OpenDraft.ODCore.ODGeometry;
using System;

namespace OpenDraft.ODCore.ODEditor
{
    public interface IEditorInputService
    {
        // Events for different input types
        event Action<Key> KeyPressed;
        event Action<ODPoint> PointProvided;
        event Action<string> TextInput;
        event Action<double> NumberInput;
        event Action CancelRequested;

        // ADD THESE METHODS to the interface
        void RaiseKeyPressed(Key key);
        void RaisePointProvided(ODPoint point);
        void RaiseTextInput(string text);
        void RaiseNumberInput(double number);
        void RaiseCancelRequested();

        // Methods to request specific input modes
        void RequestPointInput();
        void RequestTextInput();
        void CancelInputRequest();
    }
}
// IEditorInputService.cs
using Avalonia.Input;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;

namespace OpenDraft.ODCore.ODEditor
{
    public interface IODEditorInputService
    {
        // Events for different input types
        event Action<Key> KeyPressed;
        event Action<ODVec2> PointProvided;
        event Action<string> TextInput;
        event Action<double> NumberInput;
        event Action CancelRequested;
        event Action ViewportEntered;
        event Action ViewportExited;

        // ADD THESE METHODS to the interface
        void RaiseKeyPressed(Key key);
        void RaisePointProvided(ODVec2 point);
        void RaiseTextInput(string text);
        void RaiseNumberInput(double number);
        void RaiseCancelRequested();

        void RaiseViewportEntered();
        void RaiseViewportExited();

        // Methods to request specific input modes
        void RequestPointInput();
        void RequestTextInput();
        void CancelInputRequest();
    }
}
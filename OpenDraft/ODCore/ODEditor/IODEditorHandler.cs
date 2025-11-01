using Avalonia.Input;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;
using OpenDraft.ODCore.ODMath;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODEditor
{
    public interface IODEditorHandler
    {
        // Input actions
        void UpdateMousePosition(ODVec2 point, double tolerance);
        void LeftMouseClicked(bool shiftDown);
        void RightMouseClicked();
        void HandleKeyPress(Key key, KeyModifiers modifiers);

        // State queries
        ODElement? GetHighlighted();
        ODSelectionSet? GetActiveSelection();
        ODVec2 GetMousePosition(); // If you need current mouse position

        // Editor control
        void CancelCurrentCommand();
    }


    public class ODEditorHandler : IODEditorHandler
    {
        private readonly ODEditor _editor;

        public ODEditorHandler(ODEditor editor)
        {
            _editor = editor;
        }

        public void UpdateMousePosition(ODVec2 point, double tolerance)
        {
            _editor.UpdateMousePosition(point, tolerance);
        }

        public void LeftMouseClicked(bool shiftDown)
        {
            _editor.LeftMouseClicked(shiftDown);
        }

        public void RightMouseClicked()
        {
            // Future: context menu, or cancel current operation
            _editor.CancelCurrentCommand();
        }

        public void HandleKeyPress(Key key, KeyModifiers modifiers)
        {
            if (key == Key.Escape)
            {
                _editor.CancelCurrentCommand();
            }
            // Add other keyboard shortcuts as needed
        }

        public ODElement? GetHighlighted()
        {
            return _editor.GetHighlighted();
        }

        public ODSelectionSet? GetActiveSelection()
        {
            return _editor.GetActiveSelection();
        }

        public ODVec2 GetMousePosition()
        {
            return _editor.GetMousePosition();
        }

        public void CancelCurrentCommand()
        {
            _editor.CancelCurrentCommand();
        }
    }
}

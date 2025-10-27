using Avalonia.Controls;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODData
{
    public class ODSelectionSet
    {
        public List<ODElement> SelectedElements { get; }

        public ODSelectionSet()
        {
            SelectedElements = new List<ODElement>();
        }

        public ODSelectionSet(List<ODElement> elements)
        {
            SelectedElements = elements;
        }

        public bool IsEmpty => SelectedElements.Count == 0;
        public int Count => SelectedElements.Count;

        
        public void AddElement(ODElement element)
        {
            if (SelectedElements.Contains(element)) return;
            SelectedElements.Add(element);
        }

        public void AddElements(List<ODElement> elements)
        {
            foreach (var element in elements)
                AddElement(element);
        }

        public void RemoveElement(ODElement element)
        {
            if (!SelectedElements.Contains(element)) return;
            SelectedElements.Remove(element);
        }

        public ODSelectionSet Clear()
        {
            return new ODSelectionSet();
        }

        public bool Contains(ODElement element)
        {
            return SelectedElements.Contains(element);
        }
    }




    public class ODSelectionManager
    {    
        private ODSelectionSet _activeSelection;
        
        public ODSelectionManager()
        {
            _activeSelection = new ODSelectionSet();
        }

        public ODSelectionSet GetActiveSelectionSet()
        {
            if (_activeSelection == null)
                _activeSelection = new();

            return _activeSelection;
        }

        public void UpdateSelectionSet(ODSelectionSet selectionSet)
        {
            _activeSelection = selectionSet;
        }

    }

}
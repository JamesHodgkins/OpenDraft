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
        public IReadOnlyCollection<ODElement> SelectedElements { get; }

        public ODSelectionSet()
        {
            SelectedElements = new List<ODElement>().AsReadOnly();
        }

        public ODSelectionSet(IEnumerable<ODElement> elements)
        {
            SelectedElements = elements?.ToList().AsReadOnly() ?? new List<ODElement>().AsReadOnly();
        }

        public bool IsEmpty => SelectedElements.Count == 0;
        public int Count => SelectedElements.Count;

        // Helper methods for common operations
        public ODSelectionSet AddElement(ODElement element)
        {
            if (element == null) return this;

            var newElements = SelectedElements.ToList();
            if (!newElements.Contains(element))
            {
                newElements.Add(element);
            }
            return new ODSelectionSet(newElements);
        }

        public ODSelectionSet AddElements(IEnumerable<ODElement> elements)
        {
            var elementList = elements?.ToList() ?? new List<ODElement>();
            if (elementList.Count == 0) return this;

            var newElements = SelectedElements.ToList();
            foreach (var element in elementList)
            {
                if (element != null && !newElements.Contains(element))
                {
                    newElements.Add(element);
                }
            }
            return new ODSelectionSet(newElements);
        }

        public ODSelectionSet RemoveElement(ODElement element)
        {
            if (element == null) return this;

            var newElements = SelectedElements.ToList();
            newElements.Remove(element);
            return new ODSelectionSet(newElements);
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
        private readonly ODDataManager _dataManager;        
        private ODSelectionSet _activeSelection;
        
        public ODSelectionManager(ODDataManager dataManager)
        {
            _dataManager = dataManager;
            _activeSelection = new ODSelectionSet();
        }

        public ODSelectionSet GetActiveSelectionSet()
        {
            return _activeSelection;
        }

        public void UpdateSelectionSet(ODSelectionSet selectionSet)
        {
            _activeSelection = selectionSet;
        }

    }

}
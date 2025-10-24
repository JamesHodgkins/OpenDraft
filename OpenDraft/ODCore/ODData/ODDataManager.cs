using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.ObjectModel;

namespace OpenDraft.ODCore.ODData
{
    public class ODDataManager
    {
        public ObservableCollection<ODGeometry.ODElement> Elements { get; } = new();
        public ODLayerManager LayerManager { get; } = new ODLayerManager();
        public ODLineStyleRegistry LineStyleRegister { get; } = new ODLineStyleRegistry();
        public ODSymbolTable SymbolTableRegister = new ODSymbolTable();

        public ODDataManager()
        {

        }

        public void AddElement(ODElement element)
        {
            if (element == null)
                throw new ArgumentNullException(nameof(element));

            if (Elements.Contains(element))
                return;

            element.LayerId = LayerManager.GetActiveLayer();
            Elements.Add(element);
        }
    }
}

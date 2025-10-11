using CommunityToolkit.Mvvm.ComponentModel;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.Core.ODData
{
    public class ODDataManager
    {
        public ObservableCollection<ODGeometry.ODElement> Elements { get; } = new();
        public ODLayerManager LayerManager { get; } = new ODLayerManager();

        public ODDataManager()
        {

        }


        public void AddElement(ODGeometry.ODElement element)
        {
            element.LayerId = LayerManager.GetActiveLayer();
            Elements.Add(element);
        }
    }
}

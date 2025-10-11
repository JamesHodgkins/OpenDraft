using CommunityToolkit.Mvvm.ComponentModel;
using OpenDraft.ODCore.ODGeometry;
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace OpenDraft.ODCore.ODData
{
    public class ODDataManager
    {
        public ObservableCollection<ODGeometry.ODElement> Elements { get; } = new();
        public ODLayerManager LayerManager { get; } = new ODLayerManager();

        public ODDataManager()
        {

        }


        public void AddElement(ODElement element)
        {
            if (element == null)
                throw new ArgumentNullException(nameof(element));

            element.LayerId = LayerManager.GetActiveLayer();
            Elements.Add(element);
        }
    }
}

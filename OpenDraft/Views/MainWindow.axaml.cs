using Avalonia.Controls;
using OpenDraft.Core.ODData;
using OpenDraft.Core.ODGeometry;

namespace OpenDraft.Views
{
    public partial class MainWindow : Window
    {
        public ODDataManager Data { get; } = new ODDataManager();

        public MainWindow()
        {
            InitializeComponent();
        }
    }
}
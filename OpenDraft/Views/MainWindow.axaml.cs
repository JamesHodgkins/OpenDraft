using Avalonia.Controls;
using OpenDraft.ODCore.ODData;
using OpenDraft.ODCore.ODGeometry;

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
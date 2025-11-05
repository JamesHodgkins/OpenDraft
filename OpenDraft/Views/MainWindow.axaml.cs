using Avalonia.Controls;
using Avalonia.Input;
using OpenDraft.ViewModels;
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;

namespace OpenDraft.Views
{
    public partial class MainWindow : Window
    {
        public MainWindow()
        {
            InitializeComponent();

            // Subscribe to key events at window level
            AddHandler(KeyDownEvent, OnKeyDown, handledEventsToo: true);
        }

        private void OnKeyDown(object? sender, KeyEventArgs e)
        {
            if (DataContext is MainWindowViewModel vm)
            {
                // Route all keys to input service
                vm.InputService.RaiseKeyPressed(e.Key);

                // Special case: ESC also triggers cancel specifically
                if (e.Key == Key.Escape)
                {
                    vm.InputService.RaiseCancelRequested();
                    e.Handled = true;
                }
            }
        }
    }
}
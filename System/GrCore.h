#pragma once

#include "Graphics/GrGraphics.h"	// Include Graphics
#include "Graphics/GrMainWindow.h"

class GrApplication
{
private:
	bool running = false;
	GrMainWindow* mainWindow = nullptr;

public:

	GrApplication()
	{		
		mainWindow = new GrMainWindow(1280,720, "OpenDraft");
		mainWindow->initialise();
		running = true;

	}


	bool isRunning()
	{
		return running;
	}


	GrMainWindow* getMainWindow()
	{
		return mainWindow;
	}


	void update()
	{
		if (!mainWindow->isRunning())
		{
			running = false;
			return;
		}

		mainWindow->startEventsChain();
		mainWindow->render();
	}


	~GrApplication()
	{
		mainWindow->close();
		delete mainWindow;
	}

};

#pragma once

#include "Graphics/GrGraphics.h"	// Include Graphics

class GrApplication
{
private:
	bool running = false;
	GrWindow* mainWindow = nullptr;

public:

	GrApplication()
	{		
		mainWindow = new GrWindow(1280,720, "OpenDraft");
		mainWindow->initialise();
		running = true;

	}


	bool isRunning()
	{
		return running;
	}


	GrWindow* getMainWindow()
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

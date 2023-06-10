#pragma once

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyApplication.h                                                     *
* Contributors: James Hodgkins                                                        *
* Date:         June 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
***************************************************************************************/



#include "Graphics/OdGrGraphics.h"	// Include Graphics
#include "Graphics/OdGrMainWindow.h"

class OdGrApplication
{
private:
	bool running = false;
	OdGrMainWindow* mainWindow = nullptr;

public:

	OdGrApplication()
	{
		mainWindow = new OdGrMainWindow(1280, 720, "OpenDraft");
		mainWindow->initialise();
		running = true;

	}


	bool isRunning()
	{
		return running;
	}


	OdGrMainWindow* getMainWindow()
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

		mainWindow->triggerEventsChain();
		mainWindow->startFrameProcess();
	}


	~OdGrApplication()
	{
		mainWindow->close();
		delete mainWindow;
	}

};

#ifndef OD_SY_APPLICATION_H
#define OD_SY_APPLICATION_H

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


#include "Graphics/OdGrGraphics.h"		// Include Graphics
#include "Graphics/OdGrMainWindow.h"	// Include Main Window Class
#include "OdSyResourceManager.h"		// Include Resource Manager

class OdGrApplication
{
private:
	bool running = false;
	OdGrMainWindow* mainWindow = nullptr;
	OdSyResourceManager* resManager;

public:

	OdGrApplication()
	{
		resManager = new OdSyResourceManager();
		mainWindow = new OdGrMainWindow(1280, 720, "OpenDraft");
		mainWindow->resourceManager = resManager;
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
		mainWindow->onFrame();
	}


	~OdGrApplication()
	{
		mainWindow->close();
		delete resManager;
		delete mainWindow;
	}

};

#endif // OD_SY_APPLICATION_H
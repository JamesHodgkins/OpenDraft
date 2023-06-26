#ifndef OD_SY_APPLICATION_CPP
#define OD_SY_APPLICATION_CPP

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdApplication.cpp                                                     *
* Contributors: James Hodgkins                                                        *
* Date:         June 26, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
***************************************************************************************/


#include "System/OdApplication.h"


using namespace OD::Graphics;

namespace OD::System
{
	OdApplication* OdApplication::getInstance()
	{
		// Return the instance
		return instance;
	}

	OdApplication::OdApplication()
	{
		// Check if the application instance has already been created
		if (instance == nullptr)
		{
			// Otherwise, set the instance to this
			instance = this;
		}

		// Initialize the application
		resManager = new OdResourceManager();
		mainWindow = new OdMainWindow(1280, 720, "OpenDraft");
		mainWindow->resourceManager = resManager;
		mainWindow->initialise();
		running = true;
	}


	OdApplication::~OdApplication()
	{
		mainWindow->close();
		delete resManager;
		delete mainWindow;
	}


	bool OdApplication::isRunning()
	{
		return running;
	}


	OdMainWindow* OdApplication::getMainWindow()
	{
		return mainWindow;
	}


	void OdApplication::update()
	{
		if (!mainWindow->isRunning())
		{
			running = false;
			return;
		}

		mainWindow->triggerEventsChain();
		mainWindow->onFrame();
	}

	// Static reference to the application instance
	OdApplication* OdApplication::instance = nullptr;

} // namespace OD::System

#endif // OD_SY_APPLICATION_CPP
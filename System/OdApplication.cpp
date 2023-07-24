#ifndef OD_SY_APPLICATION_CPP
#define OD_SY_APPLICATION_CPP

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdApplication.cpp                                                     *
* Contributors: James Hodgkins                                                        *
* Date:         June 30, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
*                                                                                     *
* Notes:                                                                              *
*   This class is a singleton, which is currently not thread safe.                    *
***************************************************************************************/


#include "System/OdApplication.h"


using namespace OD::Graphics;

namespace OD::System
{
	OdApplication* OdApplication::getInstance()
	{
		// Check if the application instance has already been created
		if (instance_ != nullptr)
			return instance_;

		// Create the application instance
		instance_ = new OdApplication();

		// Initialize the application
		instance_->initialise();
	}

	OdApplication::OdApplication() {}


	OdApplication::~OdApplication()
	{
		mainWindow->close();
		delete resManager;
		delete mainWindow;
		delete documentManager;
	}

	void OdApplication::initialise()
	{
		// Initialize the application
		resManager = new OdResourceManager();
		documentManager = new OdDocumentManager();
		system = OdSystem::getInstance();
		mainWindow = new OdMainWindow(1280, 720, "OpenDraft");
		mainWindow->resourceManager = resManager;
		mainWindow->initialise();
		running = true;
	}


	bool OdApplication::isRunning()
	{
		return running;
	}


	// Update the application frame
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


	// Get the main window instance
	OdMainWindow* OdApplication::getMainWindow()
	{
		return mainWindow;
	}

	// Get the document manager
	OdDocumentManager* OdApplication::getDocumentManager()
	{
		return documentManager;
	}


	// Static reference to the application instance
	OdApplication* OdApplication::instance_ = nullptr;

	

} // namespace OD::System

#endif // !OD_SY_APPLICATION_CPP

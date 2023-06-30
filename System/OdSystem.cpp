#ifndef OD_SY_SYSTEM_CPP
#define OD_SY_SYSTEM_CPP

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdSystem.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 30, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Root application class                                                            *
*                                                                                     *
* Notes:                                                                              *
*   This class is a singleton, which is currently not thread safe.                    *
***************************************************************************************/


#include <string>
#include "System/OdSystem.h"


namespace OD::System
{
	// Constructor
	OdSystem::OdSystem()
	{
		// Check if the application instance has already been created
		if (instance_ == nullptr)
		{
			// Otherwise, set the instance to this
			instance_ = this;
		}
	}


	// Get the application instance
	OdSystem* OdSystem::getInstance()
	{
		// Create the instance if it does not exist
		if (instance_ == nullptr)
		{
			instance_ = new OdSystem();
		}

		// Return the instance
		return instance_;
	}


	// Destructor
	OdSystem::~OdSystem() {}


	// Get the system instance
	int OdSystem::getSystemVariable(const char* name)
	{
		//// Check if the variable exists
		//if (systemVariables.find(name) != systemVariables.end())
		//{
		//	// Return the variable value
		//	return systemVariables[name];
		//}
		//else
		//{
		//	// Return -1 if the variable does not exist
		//	return -1;
		//}
		return 0;
	}


	void OdSystem::setSystemVariable(const char* name, int value)
	{
		systemVariables[name] = value;
	}


	void OdSystem::loadSystemVariables()
	{
		// Variables are saved in an xml file called ApplicationSettings.xml
		// The file is located in the same directory as the executable

		// Check the file exists
		if (std::filesystem::exists("ApplicationSettings.xml"))
		{
			// Load the file
			pugi::xml_document doc;
			pugi::xml_parse_result result = doc.load_file("ApplicationSettings.xml");

			// Check the file loaded successfully
			if (result)
			{
				// Get the root node
				pugi::xml_node root = doc.child("ApplicationSettings");

				// Iterate through the child nodes
				for (pugi::xml_node node : root.children())
				{
					// Get the name and value of the variable
					const char* name = node.attribute("name").value();
					int value = node.attribute("value").as_int();

					// Set the variable
					OdSystem::getInstance()->setSystemVariable(name, value);
				}
			}
			else
			{
				// Print an error message if the file failed to load
				std::cout << "Error loading ApplicationSettings.xml: " << result.description() << std::endl;
			}
		
		}
		else
		{
			// Print an error message if the file does not exist
			std::cout << "Error loading ApplicationSettings.xml: File does not exist" << std::endl;
		}
	}

	// Static reference to the application instance
	OdSystem* OdSystem::instance_ = nullptr;

}// namespace OD::System

#endif // OD_SY_SYSTEM_CPP
#ifndef OD_SY_SYSTEM_CPP
#define OD_SY_SYSTEM_CPP

/**************************************************************************************
* OpenDraft:    System Application Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdSystem.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         July 01, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   System class for managing setting variables and other related tasks.              *
*                                                                                     *
* Notes:                                                                              *
*   This class is a singleton, which is currently not thread safe.                    *
***************************************************************************************/


#include <string>
#include "System/OdSystem.h"


namespace OD::System
{
	// Constructor
	OdSystem::OdSystem() {}


	// Get the application instance
	OdSystem* OdSystem::getInstance()
	{
		// Create the instance if it does not exist
		if (instance_ == nullptr)
		{
			instance_ = new OdSystem();
			instance_->loadRegistryVariablesFromFile();
		}

		// Return the instance
		return instance_;

	}


	// Destructor
	OdSystem::~OdSystem()
	{
		// Delete the registry variables
		for (auto& variable : registry)
			delete variable.second;

		// Clear the registry
		registry.clear();

		// Delete the instance
		delete instance_;
	}


	void OdSystem::loadRegistryVariablesFromFile()
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
				pugi::xml_node root = doc.child("systemVariables");

				// Iterate through the variables
				for (const auto& variable : root.children("variable"))
				{
					// Get key and value elements
					pugi::xml_node keyNode = variable.child("key");
					pugi::xml_node valNode = variable.child("value");

					// Extract key and value strings
					std::string key = keyNode.child_value();
					std::string val = valNode.child_value();
					std::string type = valNode.attribute("type").value();

					// Convert value to int or float

					// Update the registry variant value depending on type

					if (type == "int")
					{
						int iVal = std::stoi(val);
						registry[key] = new OdSystemVariable(key, iVal);
					}
					else if (type == "float")
					{
						float fVal = std::stof(val);
						registry[key] = new OdSystemVariable(key, fVal);
					}
					else if (type == "bool")
					{
						bool bVal = (val == "true");
						registry[key] = new OdSystemVariable(key, bVal);
					}
					else if (type == "string")
					{
						registry[key] = new OdSystemVariable(key, val);
					}
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

	// Get system variable
	OdSystemVariable* OdSystem::getRegistryVariableByName(const char* name)
	{
		// Return entry if it exists
		if (registry.find(name) != registry.end())
		{
			return registry[name];
		}
		else
		{
			// Return null if entry does not exist
			return nullptr;
		}
	}

	// Set system variable
	void OdSystem::updateRegistryVariable(OdSystemVariable* aEntry)
	{
		// Check if entry already exists
		if (registry.find(aEntry->getKey()) != registry.end())
		{
			// Update the entry
			registry[aEntry->getKey()] = aEntry;
		}
		else
		{
			// Add the entry
			registry[aEntry->getKey()] = aEntry;
		}
	}

	
	// Get draw line weight by index
	float OdSystem::getDrawLineWeightByIndex(int index)
	{
		OdSystemVariable* v;

			v = getRegistryVariableByName("lineThicknessDisplay0");
		
		if (index == 1)
			v = getRegistryVariableByName("lineThicknessDisplay1");

		else if (index == 2)
			v = getRegistryVariableByName("lineThicknessDisplay2");

		else if (index == 3)
			v = getRegistryVariableByName("lineThicknessDisplay3");
		
		float result = std::get<float>(v->getValue());
		return result;
	}




	// Static reference to the application instance
	OdSystem* OdSystem::instance_ = nullptr;

}// namespace OD::System

#endif // OD_SY_SYSTEM_CPP
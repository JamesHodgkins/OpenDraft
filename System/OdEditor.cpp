#ifndef OD_SY_EDITOR_CPP
#define OD_SY_EDITOR_CPP

/**************************************************************************************
* OpenDraft:    System editor class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdEditor.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         23 July, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   An editor class to handle user input/output.                                      *
***************************************************************************************/



#include <thread>
#include "System/OdEditor.h"
#include "Graphics/UI/OdInput.h"

using namespace OD::Graphics;

namespace OD::System
{
	OdEditor::OdEditor()
	{
		// Initialize the command thread
		commandThread = std::thread([this]() {
			while (!terminateFlag.load()) {
				std::this_thread::sleep_for(std::chrono::milliseconds(100));
			}
		});

		// Initialize terminateFlag atomic boolean
		terminateFlag.store(false);
	}

	OdEditor::~OdEditor()
	{

	}

	// Update events
	/*void OdEditor::processEvents(GrInputMap* input)
	{

	}*/

	// Command events
	void OdEditor::startCommand(void (*commandFunction)())
	{
		// Check if the previous thread is joinable (detached threads are not joinable)
		if (commandThread.joinable()) {
			commandThread.join(); // Join the previous thread if it is still running
		}

		// Create a background worker thread to handle the command
		commandThread = std::thread(commandFunction);
	}

	void OdEditor::endCommand()
	{
		terminateFlag.store(true); // Set the flag to signal the thread to stop
	}


	OdValuePromptResult OdEditor::getValueUserInput()
	{
		OdValuePromptResult result;

		bool shouldContinue = true;
		while (shouldContinue)
		{
			// Check if Esc key was pressed
			if (input->keys[GLFW_KEY_ESCAPE].isPressDown())
			{
				result.status = OdPromptStatus::kCancel;
				shouldContinue = false;
			}

			// Check if Enter key was pressed
			if (input->keys[GLFW_KEY_ENTER].isPressDown())
			{
				result.status = OdPromptStatus::kOK;
				shouldContinue = false;
			}
		}

		return result;
	}



	OdPointPromptResult OdEditor::getPointUserInput()
	{
		OdPointPromptResult result;

		bool shouldContinue = true;
		while (shouldContinue)
		{
			// Check if Esc key was pressed
			if (input->keys[GLFW_KEY_ESCAPE].isPressDown())
			{
				result.status = OdPromptStatus::kCancel;
				shouldContinue = false;
			}

			// Check if Enter key was pressed
			if (input->keys[GLFW_KEY_ENTER].isPressDown())
			{
				result.status = OdPromptStatus::kOK;
				shouldContinue = false;
			}
		}

		return result;
	}



	OdEntityPromptResult OdEditor::getEntityUserInput()
	{
		OdEntityPromptResult result;

		bool shouldContinue = true;
		while (shouldContinue)
		{
			// Check if Esc key was pressed
			if (input->keys[GLFW_KEY_ESCAPE].isPressDown())
			{
				result.status = OdPromptStatus::kCancel;
				shouldContinue = false;
			}

			// Check if Enter key was pressed
			if (input->keys[GLFW_KEY_ENTER].isPressDown())
			{
				result.status = OdPromptStatus::kOK;
				shouldContinue = false;
			}
		}

		return result;
	}



	OdSelectionPromptResult OdEditor::getSelectionUserInput()
	{
		OdSelectionPromptResult result;

		bool shouldContinue = true;
		while (shouldContinue)
		{
			// Check if Esc key was pressed
			if (input->keys[GLFW_KEY_ESCAPE].isPressDown())
			{
				result.status = OdPromptStatus::kCancel;
				shouldContinue = false;
			}

			// Check if Enter key was pressed
			if (input->keys[GLFW_KEY_ENTER].isPressDown())
			{
				result.status = OdPromptStatus::kOK;
				shouldContinue = false;
			}
		}

		return result;
	}



	OdStringPromptResult OdEditor::getStringUserInput()
	{
		OdStringPromptResult result;

		bool shouldContinue = true;
		while (shouldContinue)
		{
			// Check if Esc key was pressed
			if (input->keys[GLFW_KEY_ESCAPE].isPressDown())
			{
				result.status = OdPromptStatus::kCancel;
				shouldContinue = false;
			}

			// Check if Enter key was pressed
			if (input->keys[GLFW_KEY_ENTER].isPressDown())
			{
				result.status = OdPromptStatus::kOK;
				shouldContinue = false;
			}
		}

		return result;
	}



	OdIntegerPromptResult OdEditor::getIntegerUserInput()
	{
		OdIntegerPromptResult result;

		bool shouldContinue = true;
		while (shouldContinue)
		{
			// Check if Esc key was pressed
			if (input->keys[GLFW_KEY_ESCAPE].isPressDown())
			{
				result.status = OdPromptStatus::kCancel;
				shouldContinue = false;
			}

			// Check if Enter key was pressed
			if (input->keys[GLFW_KEY_ENTER].isPressDown())
			{
				result.status = OdPromptStatus::kOK;
				shouldContinue = false;
			}
		}

		return result;
	}


}

#endif // OD_SY_EDITOR_CPP
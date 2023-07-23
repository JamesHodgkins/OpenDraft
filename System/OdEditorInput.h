#ifndef OD_SY_EDITORINPUT_H
#define OD_SY_EDITORINPUT_H

/**************************************************************************************
* OpenDraft:    Editor Input Classes                                                  *
*-------------------------------------------------------------------------------------*
* Filename:     OdEditorInput.h                                                       *
* Contributors: James Hodgkins                                                        *
* Date:         July 23, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*    Classes to handle user input results.                                            *
***************************************************************************************/



#include "System/OdCore.h"


namespace OD
{
	// Forward declarations
	namespace Geometry
	{
		class OdEntity;
	}

	namespace System
	{
		class OdEditor;

	// End forward declarations



		// Create enum for prompt status
		enum class OdPromptStatus
		{
			kNone,
			kOK,
			kCancel,
			kError
		};


		// Virtual base class for user input results
		class OdPromptResult
		{
			// Set friend class
			friend class OD::System::OdEditor;

		private:

		public:
			OdPromptStatus status;

		};


		// Prompt result class for decimal values
		class OdValuePromptResult : public OdPromptResult
		{
		private:
			double value;

		public:
			double getValue() { return value; }

		};


		// Prompt result class for 2D points
		class OdPointPromptResult : public OdPromptResult
		{
		private:
			OD::System::OdVector2 value;

		public:
			OD::System::OdVector2 getValue() { return value; }

		};


		// Prompt result class for a single entity
		class OdEntityPromptResult : public OdPromptResult
		{
		private:
			OD::Geometry::OdEntity* value;

		public:
			OD::Geometry::OdEntity* getValue() { return value; }

		};


		// Prompt result class for a selection of entities
		class OdSelectionPromptResult : public OdPromptResult
		{
		private:
			std::vector<OD::Geometry::OdEntity*> value;

		public:
			std::vector<OD::Geometry::OdEntity*> getValue() { return value; }

		};


		// Prompt result class for a string
		class OdStringPromptResult : public OdPromptResult
		{
		private:
			std::string value;

		public:
			std::string getValue() { return value; }

		};


		// Prompt result class for an integer
		class OdIntegerPromptResult : public OdPromptResult
		{
		private:
			int value;

		public:
			int getValue() { return value; }

		};


	}
}

#endif // !OD_SY_EDITORINPUT_H
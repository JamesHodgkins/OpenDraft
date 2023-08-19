#ifndef OD_SY_EDITOR_H
#define OD_SY_EDITOR_H

/**************************************************************************************
* OpenDraft:    System editor class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdEditor.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         23 July, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   An editor class to handle user input/output.                                      *
***************************************************************************************/



#include "System/OdCore.h"
#include "System/OdEditorInput.h"
#include "System/Entities/OdEntity.h"


namespace OD
{
	using namespace Geometry;


	// Forward declarations
	namespace Graphics {
		class GrInputMap;
	}

	namespace Geometry {
		class OdEntity;
	}
	// End forward declarations


	namespace System
	{
		class OdEditor
		{
		private:
			Graphics::GrInputMap* input = nullptr;
			
			std::atomic<bool> terminateFlag{false};
			std::thread commandThread;

			// Selection
			std::vector<OdEntity*> selectionSet;

			

		public:
			// Constructor and destructor
			OdEditor();
			~OdEditor();

			// Update events
			//void processEvents(Graphics::GrInputMap* input);

			// Command events
			void startCommand(void (*commandFunction)());
			void endCommand();

			OdValuePromptResult getValueUserInput();
			OdPointPromptResult getPointUserInput();
			OdEntityPromptResult getEntityUserInput();
			OdSelectionPromptResult getSelectionUserInput();
			OdStringPromptResult getStringUserInput();
			OdIntegerPromptResult getIntegerUserInput();


			// Selection events
			void addEntityToSelection(OdEntity* aEntity);
			void removeEntityFromSelection(OdEntity* aEntity);
			bool isEntitySelected(OdEntity* aEntity);
			void clearSelection();


		};
	}
}

#endif // !OD_SY_EDITOR_H
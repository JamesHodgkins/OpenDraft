#ifndef OD_DAT_DOCUMENT_H
#define OD_DAT_DOCUMENT_H

/**************************************************************************************
* OpenDraft:    Document Class                                                        *
*-------------------------------------------------------------------------------------*
* Filename:     OdDocument.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/



#include <fstream>
#include <string>
#include "DataManager/OdDrawingDb.h"	// Include Drawing Database
#include "System/OdCore.h"				// Include Core Utilities


// Forward declarations
namespace OD
{
	namespace System
	{
		class OdLayer;
		class OdEditor;
	}
}

namespace OD
{
	namespace Data
	{
		// Document Class
		class OdDocument
		{
		private:


		public:

			// Document Properties
			std::string path;			// Document Path
			char version[5];			// Document Version
			std::wstring author;		// Document Author
			std::wstring company;		// Document Company
			std::wstring comments;		// Document Comments
			int creationDate;			// Document Creation Date
			int modifiedDate;			// Document Modified Date
			bool readOnly;				// Document Read-Only
			bool modified;				// Document Modified
			bool locked;				// Document Locked

			OdDrawingDb database;		// Document Database
			OdLayer* activeLayer;		// Document Active Layer

			OdEditor* editor;			// Pointer to the editor


			// Constructors and Destructors
			OdDocument();
			~OdDocument();

			
			// 
			// Getters and Setters
			//

			// Document Path
			std::string getPath();
			void setPath(std::string aPath);
			
			// Document Version
			std::string getVersion();
			void setVersion(const char* aVersion);

			// Document Read-Only
			bool isReadOnly();
			bool upgradeOpen();

			// Document Modified
			bool isModified();
			void setModified(bool aModified);

			// Document Locked
			bool isLocked();
			void setLocked(bool aLocked);

			// Document Save
			bool save();

			// Document Save As
			bool saveAs(std::string aPath);

			// Create new Document
			bool createNew(std::string aPath = "");

			// Update Document
			bool updateDocument();

			// Get Document Database
			OdDrawingDb* getDatabase();

			// Get Document Active Layer
			OdLayer* getActiveLayer();

			// Get Document Editor
			OdEditor* getEditor();

			//
			// File structure methods
			//

			std::vector<wchar_t> compileFileHeader();

			std::vector<wchar_t> compileFileDatabase();


		};

	}// namespace OD::Data
} // namespace OD

#endif // !OD_SY_DOCUMENT_H

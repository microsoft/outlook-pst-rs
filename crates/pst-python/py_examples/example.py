import argparse
import pst_python
from pst_python import PyStore, PyFolder, PyMessage


def print_separator(title: str = ""):
    """Print separator line"""
    if title:
        print(f"\n{'=' * 60}")
        print(f"  {title}")
        print(f"{'=' * 60}")
    else:
        print("-" * 60)


def example_store_properties(store: PyStore):
    """Display store properties"""
    print_separator("Store Properties")
    props = store.properties()

    print(f"Display Name: {props.display_name()}")
    print(f"Unique Value: {store.unique_value()}")

    ipm_sub_tree = props.ipm_sub_tree_entry_id()
    print(f"IPM Sub Tree Entry ID: {ipm_sub_tree}")

    print("\nStore Properties (first 5):")
    props_dict = props.iter()
    for i, (key, value) in enumerate(props_dict.items()):
        if i >= 5:
            break
        print(f"  {key}: {value}")


def example_named_property_map(store: PyStore):
    """Display named property map"""
    print_separator("Named Property Map")

    try:
        named_map = store.named_property_map()
        props = named_map.properties()

        print(f"Bucket Count: {props.bucket_count()}")

        stream_guid = props.stream_guid()
        print(f"Stream GUIDs: {len(stream_guid)}")

        stream_entry = props.stream_entry()
        print(f"Stream Entries: {len(stream_entry)}")

        if stream_entry:
            print("\nFirst 3 stream entries:")
            for entry in stream_entry[:3]:
                print(f"  {entry}")
    except Exception as e:
        print(f"Error accessing named property map: {e}")


def explore_all_folders_and_messages(store: PyStore):
    """Recursively explore all folders and messages"""
    print_separator("Full PST Exploration")

    try:
        props = store.properties()
        ipm_sub_tree = props.ipm_sub_tree_entry_id()
        node_id_str = ipm_sub_tree.get('node_id', '')

        if not node_id_str:
            print("Error: Could not get IPM Subtree Entry ID")
            return

        node_id_hex = node_id_str.replace('0x', '').replace('0X', '')
        root_folder = store.open_folder(node_id_hex)

        print(f"Starting exploration from IPM Subtree (Node ID: {node_id_hex})")
        print()

        _explore_folder(store, root_folder, 0)

    except Exception as e:
        print(f"Error during exploration: {e}")
        import traceback
        traceback.print_exc()


def _explore_folder(store: PyStore, folder: PyFolder, indent: int):
    """Recursively explore folder"""
    indent_str = "  " * indent
    props = folder.properties()

    try:
        folder_name = props.display_name()
        node_id = props.node_id()
        print(f"{indent_str}📁 Folder: {folder_name} (Node ID: {node_id})")

        try:
            content_count = props.content_count()
            unread_count = props.unread_count()
            has_sub_folders = props.has_sub_folders()
            print(f"{indent_str}  Content Count: {content_count}, Unread: {unread_count}, Has Sub Folders: {has_sub_folders}")
        except Exception:
            pass

        hierarchy_table = folder.hierarchy_table()
        if hierarchy_table:
            rows = hierarchy_table.rows_matrix()
            sub_folder_count = 0

            for row in rows:
                try:
                    row_id = row.get('id')
                    if row_id is None:
                        continue

                    node_id_hex = f"{row_id:X}"
                    sub_folder = store.open_folder(node_id_hex)
                    _explore_folder(store, sub_folder, indent + 1)
                    sub_folder_count += 1
                except Exception:
                    pass

            if sub_folder_count > 0:
                print(f"{indent_str}  Total sub folders: {sub_folder_count}")

        contents_table = folder.contents_table()
        if contents_table:
            rows = contents_table.rows_matrix()
            message_count = 0

            for row in rows:
                try:
                    row_id = row.get('id')
                    if row_id is None:
                        continue

                    node_id_hex = f"{row_id:X}"
                    message = store.open_message(node_id_hex)
                    _explore_message(store, message, indent + 1)
                    message_count += 1
                except Exception:
                    pass

            if message_count > 0:
                print(f"{indent_str}  Total messages: {message_count}")

        print()

    except Exception as e:
        print(f"{indent_str}Error exploring folder: {e}")


def _explore_message(store: PyStore, message: PyMessage, indent: int):
    """Display message information"""
    indent_str = "  " * indent
    props = message.properties()

    try:
        subject = "(no subject)"
        props_dict = props.iter()
        subject_prop = props_dict.get('0x0037')
        if subject_prop:
            subject = str(subject_prop)
            if len(subject) > 60:
                subject = subject[:60] + "..."

        print(f"{indent_str}📧 Message: {subject}")

        try:
            message_class = props.message_class()
            print(f"{indent_str}  Class: {message_class}")
        except Exception:
            pass

        try:
            creation_time = props.creation_time()
            print(f"{indent_str}  Created: {creation_time}")
        except Exception:
            pass

        try:
            recipient_table = message.recipient_table()
            recipients = recipient_table.rows_matrix()
            if len(recipients) > 0:
                print(f"{indent_str}  Recipients: {len(recipients)}")
        except Exception:
            pass

        try:
            attachment_table = message.attachment_table()
            if attachment_table:
                attachments = attachment_table.rows_matrix()
                if len(attachments) > 0:
                    print(f"{indent_str}  Attachments: {len(attachments)}")
        except Exception:
            pass

    except Exception as e:
        print(f"{indent_str}Error exploring message: {e}")


def main():
    parser = argparse.ArgumentParser(
        description="PST Python binding example"
    )
    parser.add_argument(
        "--pst-path",
        type=str,
        required=True,
        help="Path to PST file"
    )
    args = parser.parse_args()

    print("PST Python Binding Example")
    print(f"PST File: {args.pst_path}")

    store: PyStore = pst_python.open_pst(args.pst_path)

    example_store_properties(store)
    example_named_property_map(store)
    explore_all_folders_and_messages(store)

    print_separator()
    print("Done!")


if __name__ == "__main__":
    main()
